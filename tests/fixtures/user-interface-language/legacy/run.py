from pathlib import Path
import hashlib, json, os, shutil, socket, subprocess, tempfile, time, uuid
import psycopg2

root = Path.cwd()
proof = Path(__file__).resolve().parent
work = Path(tempfile.mkdtemp(dir=root/'target', prefix='language-legacy-native-'))
pg = Path('/opt/pgpro/ent-18/bin')
platform = Path('/opt/1cv8/x86_64/8.3.27.2342')
native = root/'docs/research/fixtures/approved-sdk-production-integration-1016/current-language-session-audit/native-user-records-1017'
snapshot_hash = hashlib.sha256((native/'languages.dt').read_bytes()).hexdigest()
database = 'native_legacy_user_languages_1018'
environment = os.environ | dict(LD_LIBRARY_PATH='/home/kan/.local/lib/1c-wk41:/usr/lib', GDK_BACKEND='x11')
environment.pop('WAYLAND_DISPLAY', None)
(work/'tmp').mkdir()
for key in ['TMPDIR', 'TMP', 'TEMP']:
    environment[key] = str(work/'tmp')
checks = []
started = False
server = None
backup = None

def run(label, args, timeout=180):
    start = time.monotonic()
    with (proof/(label+'.log')).open('w') as log:
        result = subprocess.run([str(x) for x in args], env=environment, stdout=log, stderr=subprocess.STDOUT, timeout=timeout)
    checks.append(dict(check=label, exit_code=result.returncode, seconds=round(time.monotonic()-start,2)))
    (proof/'checks.json').write_text(json.dumps(checks,indent=2)+'\n')
    print(checks[-1], flush=True)
    assert result.returncode == 0, label

def free_port():
    with socket.socket() as listener:
        listener.bind(('127.0.0.1',0))
        return listener.getsockname()[1]

port = free_port()

def connect():
    connection = psycopg2.connect(host='127.0.0.1',port=port,dbname=database,user='native_control')
    with connection.cursor() as query:
        query.execute('SELECT current_database()')
        assert query.fetchone()[0] == database
    return connection

def stop_server():
    global server
    if server is not None:
        server.terminate()
        try:
            server.wait(timeout=10)
        except subprocess.TimeoutExpired:
            server.kill()
            server.wait(timeout=5)
        server = None

try:
    run('initdb',[pg/'initdb','-D',work/'data','-U','native_control','--auth=trust','--locale=en_US.utf8','-E','UTF8'])
    run('pg-start',[pg/'pg_ctl','-D',work/'data','-l',proof/'postgres.log','-o',f'-h 127.0.0.1 -p {port} -c unix_socket_directories= -F -c shared_buffers=16MB -c max_connections=20','-w','start'])
    started = True
    base = ['--dbms=PostgreSQL',f'--db-server=127.0.0.1 port={port}',f'--db-name={database}','--db-user=native_control',f'--data={work}/management']
    run('restore',[platform/'ibcmd','infobase','restore',*base,'--create-database',native/'languages.dt'])
    base += ['--user=LangAdmin','--password=']
    run('export',[platform/'ibcmd','config','export',*base,work/'xml'])
    module = work/'xml/DataProcessors/Замеры/Forms/Форма/Ext/Form/Module.bsl'
    assert module.is_file(), 'expected own measurement form'
    module.write_text((proof/'Module.bsl').read_text().replace('__RESULT_DIRECTORY__',str(proof)),encoding='utf-8-sig')
    (work/'xml/Ext/ManagedApplicationModule.bsl').write_text('Процедура ПриНачалеРаботыСистемы()\nОткрытьФорму("Обработка.Замеры.Форма");\nКонецПроцедуры\n',encoding='utf-8-sig')
    run('import',[platform/'ibcmd','config','import',*base,work/'xml'])
    run('apply',[platform/'ibcmd','config','apply',*base,'--dynamic=disable','--session-terminate=disable','--force'])
    names = ['LangRussian','LangEnglish','LangDefault']
    with connect() as connection:
        with connection.cursor() as query:
            query.execute('SELECT name::text,data FROM v8users WHERE name::text=ANY(%s)',(names,))
            backup = {name:bytes(data) for name,data in query.fetchall()}
    assert set(backup) == set(names)
    (proof/'baseline').mkdir(exist_ok=True)
    for name in names:
        (proof/'baseline'/f'{name}.data').write_bytes(backup[name])
    # Официальное восстановление не обещает сохранить упаковку Data.
    # Генератор сверяет идентификаторы, имена и роли через прежний SDK.
    run('prepare-verified-records',['python3',proof/'prepare-records.py'])
    # Порты выделяем только на loopback; старые собственные серверы не трогаем.
    http_port, monitor_port = free_port(), free_port()
    registry = free_port()
    direct_start = free_port()
    assert direct_start+7 < 65536
    for candidate in range(direct_start,direct_start+8):
        with socket.socket() as listener:
            listener.bind(('127.0.0.1',candidate))
    config = work/'server.yml'
    config.write_text(f'server:\n  address: localhost\n  port: {http_port}\ndatabase:\n  dbms: PostgreSQL\n  server: 127.0.0.1 port={port}\n  name: {database}\n  user: native_control\ninfobase:\n  id: {uuid.uuid4()}\n  name: LegacyLanguageOwn\n  distribute-licenses: yes\n  schedule-jobs: deny\nhttp:\n  base: /\n')
    results = []
    for layout in [37,31]:
        expected_records = {name:(proof/'inputs'/f'{name}.legacy-{layout}.data').read_bytes() for name in names}
        with connect() as connection:
            with connection.cursor() as query:
                for name,data in expected_records.items():
                    query.execute('UPDATE v8users SET data=%s WHERE name::text=%s',(psycopg2.Binary(data),name))
                    assert query.rowcount == 1
        with (proof/f'server-{layout}.log').open('w') as log:
            server = subprocess.Popen([str(platform/'ibsrv'),f'--config={config}',f'--data={work}/server-data','--http-address=localhost',f'--http-port={http_port}',f'--direct-regport={registry}',f'--direct-range={direct_start}:{direct_start+7}','--monitor-address=localhost',f'--monitor-port={monitor_port}','--disable-ssh-gate','--disable-direct-gate','--schedule-jobs=deny'],env=environment,stdout=log,stderr=subprocess.STDOUT)
        deadline = time.monotonic()+20
        while True:
            assert server.poll() is None, 'own native server exited'
            try:
                with socket.create_connection(('127.0.0.1',http_port),timeout=.2):
                    break
            except OSError:
                assert time.monotonic() < deadline, 'own native gateway not ready'
                time.sleep(.2)
        oracle = proof/f'legacy-language-{layout}.oracle'
        assert not oracle.exists(), 'do not reuse native oracle'
        run(f'client-{layout}',[platform/'1cv8c','ENTERPRISE','/N','LangAdmin','/WS',f'http://127.0.0.1:{http_port}/','/C',str(layout),'/RunModeManagedApplication','/DisableStartupDialogs','/Out',proof/f'client-{layout}.runtime.log'],timeout=90)
        stop_server()
        actual = oracle.read_text(encoding='utf-8-sig')
        assert 'ERROR=' not in actual
        for line in ['LangRussian=Русский','LangEnglish=English','LangDefault=<undefined>','PRIMARY=English','CURRENT=English','ALIASES=YES','completed=YES']:
            assert line in actual.splitlines(), line
        with connect() as connection:
            with connection.cursor() as query:
                query.execute('SELECT name::text,data FROM v8users WHERE name::text=ANY(%s)',(names,))
                after = {name:bytes(data) for name,data in query.fetchall()}
        assert after == expected_records, 'native read normalized or changed legacy input'
        results.append(dict(layout=layout,status='passed',records_unchanged=True,oracle=oracle.name))
        (proof/'result.json').write_text(json.dumps(dict(status='partial' if len(results)<2 else 'passed',cases=results,production_database_modified=False,original_dt_unchanged=hashlib.sha256((native/'languages.dt').read_bytes()).hexdigest()==snapshot_hash,scope='Controlled reconstruction of own native user records; official 1C user-manager Language read. Not an original dump from an old platform version.'),indent=2)+'\n')
finally:
    stop_server()
    if backup is not None and started:
        with connect() as connection:
            with connection.cursor() as query:
                for name,data in backup.items():
                    query.execute('UPDATE v8users SET data=%s WHERE name::text=%s',(psycopg2.Binary(data),name))
    if started:
        run('pg-stop',[pg/'pg_ctl','-D',work/'data','-m','fast','-w','stop'])
    shutil.rmtree(work)

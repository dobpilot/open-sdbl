from pathlib import Path
import os,subprocess,shutil,json,time,re
w=Path('/tmp/sdbl-extension-rights-evidence-1013');env=os.environ.copy();env['LD_LIBRARY_PATH']='/home/kan/.local/lib/1c-wk41:/usr/lib';env['GDK_BACKEND']='x11';env.pop('WAYLAND_DISPLAY',None);platform='/opt/1cv8/x86_64/8.3.27.2342';receipts=[]
for mode,source in [('empty','empty-native-xml')]:
 x=w/(mode+'-users5-input');shutil.copytree(w/source,x)
 p=x/'Configuration.xml';t=p.read_text(encoding='utf-8-sig');t=t.replace('<ChildObjects>','<ChildObjects><Role>ControlClient</Role><Role>ControlAdmin</Role><CommonModule>ControlNative</CommonModule>',1);p.write_text(t,encoding='utf-8-sig')
 (x/'Roles/ControlClient.xml').write_text('<MetaDataObject xmlns="http://v8.1c.ru/8.3/MDClasses" version="2.20"><Role uuid="55555555-5555-4555-8555-555555555555"><Properties><Name>ControlClient</Name><Synonym/><Comment/></Properties></Role></MetaDataObject>')
 d=x/'Roles/ControlClient/Ext';d.mkdir(parents=True)
 (d/'Rights.xml').write_text('<Rights xmlns="http://v8.1c.ru/8.2/roles" version="2.20"><setForNewObjects>false</setForNewObjects><setForAttributesByDefault>false</setForAttributesByDefault><independentRightsOfChildObjects>false</independentRightsOfChildObjects><object><name>Configuration.Configuration</name>'+''.join('<right><name>'+name+'</name><value>true</value></right>' for name in ['ThinClient','ThickClient','MainWindowModeNormal'])+'</object></Rights>')
 (x/'Roles/ControlAdmin.xml').write_text((x/'Roles/ControlClient.xml').read_text().replace('ControlClient','ControlAdmin').replace('55555555-5555-4555-8555-555555555555','77777777-7777-4777-8777-777777777777'))
 admin=x/'Roles/ControlAdmin/Ext';admin.mkdir(parents=True);(admin/'Rights.xml').write_text((d/'Rights.xml').read_text().replace('</object>','<right><name>Administration</name><value>true</value></right></object>'))
 d=x/'CommonModules';d.mkdir(exist_ok=True)
 template=Path('/tmp/open-sdbl-extension-rights-recon-1013/tests/fixtures/functional-option-content-native/xml/CommonModules/ЗамерыЗначенияМодуля.xml').read_text(encoding='utf-8-sig').replace('ЗамерыЗначенияМодуля','ControlNative').replace('4d392458-2a40-4ad6-9c35-9c8422c2184b','66666666-6666-4666-8666-666666666666')
 (d/'ControlNative.xml').write_text(template);d=d/'ControlNative/Ext';d.mkdir(parents=True)
 script='''Процедура ВыполнитьКонтроль() Экспорт
    ПисательКонтроля = Новый ЗаписьТекста("OUTPUT", "UTF-8");
    Попытка
        Если ПользователиИнформационнойБазы.НайтиПоИмени("Baseline") = Неопределено Тогда
            ИменаКонтроля = Новый Массив;
            ИменаКонтроля.Добавить("ControlAdministrator");
            ИменаКонтроля.Добавить("Baseline");
            ИменаКонтроля.Добавить("Target");
            Для Каждого ИмяКонтроля Из ИменаКонтроля Цикл
                ПользовательКонтроля = ПользователиИнформационнойБазы.СоздатьПользователя();
                ПользовательКонтроля.Имя = ИмяКонтроля;
                ПользовательКонтроля.АутентификацияСтандартная = Истина;
                Если ИмяКонтроля = "ControlAdministrator" Тогда
                    ПользовательКонтроля.Роли.Добавить(Метаданные.Роли.ControlAdmin);
                Иначе
                    ПользовательКонтроля.Роли.Добавить(Метаданные.Роли.ControlClient);
                КонецЕсли;
                Если ИмяКонтроля = "Target" Тогда
                    ПользовательКонтроля.Роли.Добавить(Метаданные.Роли.ДемоПроф_ОбщиеПрава);
                КонецЕсли;
                ПользовательКонтроля.Записать();
            КонецЦикла;
            ПисательКонтроля.ЗаписатьСтроку("bootstrap=complete");
        Иначе
            ПисательКонтроля.ЗаписатьСтроку("user=" + ИмяПользователя());
            ПисательКонтроля.ЗаписатьСтроку("role=" + Строка(РольДоступна("ДемоПроф_ОбщиеПрава")));
            ИменаПравКонтроля = Новый Массив;
            ИменаПравКонтроля.Добавить("Администрирование");
            ИменаПравКонтроля.Добавить("АдминистрированиеДанных");
            ИменаПравКонтроля.Добавить("ОбновлениеКонфигурацииБазыДанных");
            ИменаПравКонтроля.Добавить("АктивныеПользователи");
            ИменаПравКонтроля.Добавить("ЖурналРегистрации");
            ИменаПравКонтроля.Добавить("ТонкийКлиент");
            ИменаПравКонтроля.Добавить("ТолстыйКлиент");
            ИменаПравКонтроля.Добавить("Вывод");
            Для Каждого ИмяПраваКонтроля Из ИменаПравКонтроля Цикл
                Попытка
                    ПисательКонтроля.ЗаписатьСтроку(ИмяПраваКонтроля + "=" + Строка(ПравоДоступа(ИмяПраваКонтроля, Метаданные)));
                Исключение
                    ПисательКонтроля.ЗаписатьСтроку(ИмяПраваКонтроля + "=ERROR:" + ОписаниеОшибки());
                КонецПопытки;
            КонецЦикла;
        КонецЕсли;
    Исключение
        ПисательКонтроля.ЗаписатьСтроку("ERROR:" + ОписаниеОшибки());
    КонецПопытки;
    ПисательКонтроля.ЗаписатьСтроку("complete=true");
    ПисательКонтроля.Закрыть();
КонецПроцедуры
'''.replace('OUTPUT',str(w/(mode+'-users-runtime.txt')))
 script=script.replace('Процедура ВыполнитьКонтроль() Экспорт','Функция ВыполнитьКонтроль() Экспорт').replace('КонецПроцедуры','КонецФункции')
 script=re.sub(r'    ПисательКонтроля = Новый ЗаписьТекста\([^\n]+\);','    РезультатКонтроля = "server=entered" + Символы.ПС;',script)
 script=script.replace('Если ПользователиИнформационнойБазы.НайтиПоИмени("Baseline") = Неопределено Тогда','Если ИмяПользователя() = "" Тогда')
 script=re.sub(r'ПисательКонтроля\.ЗаписатьСтроку\((.*)\);',r'РезультатКонтроля = РезультатКонтроля + \1 + Символы.ПС;',script)
 script=script.replace('ПисательКонтроля.Закрыть();','Возврат РезультатКонтроля;')
 (d/'Module.bsl').write_text(script)
 d=x/'Ext';d.mkdir(exist_ok=True)
 (d/'ManagedApplicationModule.bsl').write_text('''Процедура ПриНачалеРаботыСистемы()
    ПисательКонтроля = Новый ЗаписьТекста("OUTPUT", "UTF-8");
    ПисательКонтроля.ЗаписатьСтроку("client=entered");
    ПисательКонтроля.Закрыть();
    Попытка
        РезультатКонтроля = ControlNative.ВыполнитьКонтроль();
    Исключение
        РезультатКонтроля = "ERROR:server-call:" + ОписаниеОшибки();
    КонецПопытки;
    ПисательКонтроля = Новый ЗаписьТекста("OUTPUT", "UTF-8", , Истина);
    ПисательКонтроля.ЗаписатьСтроку(РезультатКонтроля);
    ПисательКонтроля.ЗаписатьСтроку("client=complete");
    ПисательКонтроля.Закрыть();
    ЗавершитьРаботуСистемы(Ложь);
КонецПроцедуры
'''.replace('OUTPUT',str(w/(mode+'-users-runtime.txt'))))
 db=w/(mode+'-users5-ib')
 commands=[('create',[platform+'/ibcmd','infobase','create','--db-path='+str(db),'--create-database']),('load',[platform+'/1cv8','DESIGNER','/F',str(db),'/LoadConfigFromFiles',str(x)]),('update',[platform+'/1cv8','DESIGNER','/F',str(db),'/UpdateDBCfg']),('bootstrap',[platform+'/1cv8','ENTERPRISE','/F',str(db)]),('Baseline',[platform+'/1cv8','ENTERPRISE','/F',str(db),'/N','Baseline']),('Target',[platform+'/1cv8','ENTERPRISE','/F',str(db),'/N','Target'])]
 for label,cmd in commands:
  result=w/(mode+'-users-runtime.txt')
  if label in ['bootstrap','Baseline','Target']:result.unlink(missing_ok=True)
  if 'ENTERPRISE' in cmd:cmd+=['/RunModeManagedApplication']
  cmd+=['/DisableStartupDialogs','/Out',str(w/(mode+'-users5-'+label+'.log'))] if 'DESIGNER' in cmd else []
  start=time.monotonic()
  with (w/(mode+'-users5-'+label+'.stdout.log')).open('wb') as out:r=subprocess.run(cmd,env=env,stdout=out,stderr=subprocess.STDOUT,timeout=60)
  receipts.append({'mode':mode,'label':label,'command':cmd,'exit':r.returncode,'seconds':round(time.monotonic()-start,2)});(w/'users5-receipts.json').write_text(json.dumps(receipts,indent=2));print(mode,label,r.returncode,flush=True)
  result=w/(mode+'-users-runtime.txt')
  if label in ['bootstrap','Baseline','Target'] and result.exists():shutil.copyfile(result,w/(mode+'-user-'+label+'.txt'))
  if label in ['bootstrap','Baseline','Target']:
   assert result.exists() and 'complete=true' in result.read_text(encoding='utf-8-sig'), 'No completed native oracle'
   assert 'ERROR:' not in result.read_text(encoding='utf-8-sig'), 'Native control failed'
   if label in ['Baseline','Target']:assert 'user='+label in result.read_text(encoding='utf-8-sig'), 'Wrong native user context'
  if r.returncode:raise SystemExit(r.returncode)

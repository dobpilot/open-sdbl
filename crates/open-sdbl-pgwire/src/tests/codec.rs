use super::*;

/// Encodes frontend messages the way a client does.
fn string(body: &mut Vec<u8>, value: &str) {
    body.extend_from_slice(value.as_bytes());
    body.push(0);
}

fn encode(message: &FrontendMessage) -> (u8, Vec<u8>) {
    let mut body = Vec::new();
    let code = match message {
        FrontendMessage::Query(text) => {
            string(&mut body, text);
            b'Q'
        }
        FrontendMessage::Parse {
            name,
            query,
            parameter_types,
        } => {
            string(&mut body, name);
            string(&mut body, query);
            body.extend_from_slice(&i16::try_from(parameter_types.len()).unwrap().to_be_bytes());
            for oid in parameter_types {
                body.extend_from_slice(&oid.to_be_bytes());
            }
            b'P'
        }
        FrontendMessage::Bind {
            portal,
            statement,
            parameter_formats,
            parameters,
            result_formats,
        } => {
            string(&mut body, portal);
            string(&mut body, statement);
            body.extend_from_slice(
                &i16::try_from(parameter_formats.len())
                    .unwrap()
                    .to_be_bytes(),
            );
            for format in parameter_formats {
                body.extend_from_slice(&format.to_be_bytes());
            }
            body.extend_from_slice(&i16::try_from(parameters.len()).unwrap().to_be_bytes());
            for value in parameters {
                match value {
                    None => body.extend_from_slice(&(-1_i32).to_be_bytes()),
                    Some(bytes) => {
                        body.extend_from_slice(&i32::try_from(bytes.len()).unwrap().to_be_bytes());
                        body.extend_from_slice(bytes);
                    }
                }
            }
            body.extend_from_slice(&i16::try_from(result_formats.len()).unwrap().to_be_bytes());
            for format in result_formats {
                body.extend_from_slice(&format.to_be_bytes());
            }
            b'B'
        }
        FrontendMessage::Describe { kind, name } | FrontendMessage::Close { kind, name } => {
            body.push(match kind {
                TargetKind::Statement => b'S',
                TargetKind::Portal => b'P',
            });
            string(&mut body, name);
            if matches!(message, FrontendMessage::Describe { .. }) {
                b'D'
            } else {
                b'C'
            }
        }
        FrontendMessage::Execute { portal, max_rows } => {
            string(&mut body, portal);
            body.extend_from_slice(&i32::try_from(*max_rows).unwrap().to_be_bytes());
            b'E'
        }
        FrontendMessage::Flush => b'H',
        FrontendMessage::Sync => b'S',
        FrontendMessage::Terminate => b'X',
        FrontendMessage::Password(password) => {
            string(&mut body, password);
            b'p'
        }
        FrontendMessage::Unsupported(code) => *code,
    };
    (code, body)
}

#[test]
fn frontend_messages_round_trip() {
    let messages = vec![
        FrontendMessage::Query("ВЫБРАТЬ 1".to_owned()),
        FrontendMessage::Parse {
            name: "s1".to_owned(),
            query: "ВЫБРАТЬ &А".to_owned(),
            parameter_types: vec![0, 25, 1114],
        },
        FrontendMessage::Bind {
            portal: String::new(),
            statement: "s1".to_owned(),
            parameter_formats: vec![1],
            parameters: vec![Some(b"x".to_vec()), None, Some(Vec::new())],
            result_formats: vec![0, 1],
        },
        FrontendMessage::Describe {
            kind: TargetKind::Statement,
            name: "s1".to_owned(),
        },
        FrontendMessage::Describe {
            kind: TargetKind::Portal,
            name: String::new(),
        },
        FrontendMessage::Execute {
            portal: "p".to_owned(),
            max_rows: 100,
        },
        FrontendMessage::Close {
            kind: TargetKind::Portal,
            name: "p".to_owned(),
        },
        FrontendMessage::Flush,
        FrontendMessage::Sync,
        FrontendMessage::Terminate,
        FrontendMessage::Password(zeroize::Zeroizing::new("пароль".to_owned())),
    ];
    for message in messages {
        let (code, body) = encode(&message);
        assert_eq!(decode_frontend(code, &body).unwrap(), message);
    }
}

#[test]
fn copy_and_function_calls_are_unsupported_not_malformed() {
    for code in *b"dcfF" {
        assert_eq!(
            decode_frontend(code, b"anything").unwrap(),
            FrontendMessage::Unsupported(code)
        );
    }
}

#[test]
fn negative_execute_limit_is_no_limit() {
    let mut body = b"p\0".to_vec();
    body.extend_from_slice(&(-5_i32).to_be_bytes());
    assert_eq!(
        decode_frontend(b'E', &body).unwrap(),
        FrontendMessage::Execute {
            portal: "p".to_owned(),
            max_rows: 0
        }
    );
}

#[test]
fn malformed_frontend_messages_are_rejected() {
    let cases: Vec<(u8, Vec<u8>)> = vec![
        // Unterminated strings.
        (b'Q', "ВЫБРАТЬ".as_bytes().to_vec()),
        (b'P', b"name\0query".to_vec()),
        // Invalid UTF-8.
        (b'Q', vec![0xff, 0xfe, 0]),
        // Trailing bytes.
        (b'Q', b"x\0y".to_vec()),
        (b'S', b"x".to_vec()),
        // Truncated counts and values.
        (b'P', b"n\0q\0\x00".to_vec()),
        (b'P', b"n\0q\0\x00\x02\x00\x00\x00\x19".to_vec()),
        (b'B', b"\0s\0\x00\x00\x00\x01\x00\x00\x00\x05ab".to_vec()),
        // Negative counts and lengths.
        (b'P', b"n\0q\0\xff\xff".to_vec()),
        (
            b'B',
            b"\0s\0\x00\x00\x00\x01\xff\xff\xff\xfe\x00\x00".to_vec(),
        ),
        // Describe of something that is neither statement nor portal.
        (b'D', b"Xname\0".to_vec()),
        (b'C', Vec::new()),
        (b'E', b"p\0\x00".to_vec()),
    ];
    for (code, body) in cases {
        assert!(
            decode_frontend(code, &body).is_err(),
            "{} {body:?}",
            char::from(code)
        );
    }
}

#[test]
fn password_is_not_debug_printed() {
    let message = FrontendMessage::Password(zeroize::Zeroizing::new("PaSs123!".to_owned()));
    assert!(!format!("{message:?}").contains("PaSs123!"));
}

fn startup_body(version: u32, parameters: &[(&str, &str)]) -> Vec<u8> {
    let mut body = version.to_be_bytes().to_vec();
    for (name, value) in parameters {
        string(&mut body, name);
        string(&mut body, value);
    }
    body.push(0);
    body
}

#[test]
fn startup_packets_decode() {
    assert_eq!(
        decode_startup(&80_877_103_u32.to_be_bytes()).unwrap(),
        StartupPacket::SslRequest
    );
    assert_eq!(
        decode_startup(&80_877_104_u32.to_be_bytes()).unwrap(),
        StartupPacket::GssEncRequest
    );
    let mut cancel = 80_877_102_u32.to_be_bytes().to_vec();
    cancel.extend_from_slice(&7_u32.to_be_bytes());
    cancel.extend_from_slice(&[1, 2, 3, 4]);
    assert_eq!(
        decode_startup(&cancel).unwrap(),
        StartupPacket::CancelRequest {
            process_id: 7,
            secret: vec![1, 2, 3, 4]
        }
    );
    assert_eq!(
        decode_startup(&startup_body(
            0x0003_0000,
            &[("user", "Петрова"), ("database", "unf")]
        ))
        .unwrap(),
        StartupPacket::Startup {
            major: 3,
            minor: 0,
            parameters: vec![
                ("user".to_owned(), "Петрова".to_owned()),
                ("database".to_owned(), "unf".to_owned())
            ]
        }
    );
    assert_eq!(
        decode_startup(&startup_body(0x0003_0002, &[])).unwrap(),
        StartupPacket::Startup {
            major: 3,
            minor: 2,
            parameters: Vec::new()
        }
    );
}

#[test]
fn malformed_startup_packets_are_rejected() {
    let mut unterminated = 0x0003_0000_u32.to_be_bytes().to_vec();
    unterminated.extend_from_slice(b"user\0x");
    let mut cancel_without_key = 80_877_102_u32.to_be_bytes().to_vec();
    cancel_without_key.extend_from_slice(&7_u32.to_be_bytes());
    let mut ssl_trailing = 80_877_103_u32.to_be_bytes().to_vec();
    ssl_trailing.push(0);
    let mut extra = startup_body(0x0003_0000, &[]);
    extra.push(b'x');
    for body in [
        Vec::new(),
        vec![0, 3],
        unterminated,
        cancel_without_key,
        ssl_trailing,
        extra,
        vec![0; MAX_STARTUP_PACKET],
    ] {
        assert!(decode_startup(&body).is_err(), "{body:?}");
    }
}

/// Splits encoded backend messages into `(type, body)`.
fn frames(bytes: &[u8]) -> Vec<(u8, Vec<u8>)> {
    let mut frames = Vec::new();
    let mut rest = bytes;
    while !rest.is_empty() {
        let length = u32::from_be_bytes([rest[1], rest[2], rest[3], rest[4]]) as usize;
        frames.push((rest[0], rest[5..1 + length].to_vec()));
        rest = &rest[1 + length..];
    }
    frames
}

#[test]
fn backend_messages_are_framed() {
    let mut out = BackendWriter::new();
    out.authentication_cleartext();
    out.authentication_ok();
    out.parameter_status("client_encoding", "UTF8");
    out.backend_key_data(42, 0xdead_beef);
    out.ready_for_query(TransactionStatus::Block);
    out.row_description(&[FieldDescription {
        name: "Наименование".to_owned(),
        type_oid: 25,
        type_length: -1,
        type_modifier: -1,
        format: 0,
    }]);
    out.data_row([Some(b"abc".as_slice()), None]);
    out.command_complete("SELECT 1");
    out.empty_query();
    out.parse_complete();
    out.bind_complete();
    out.close_complete();
    out.no_data();
    out.portal_suspended();
    out.parameter_description(&[25, 1114]);
    out.negotiate_protocol_version(&["_pq_.x".to_owned()]);
    let mut error = ErrorFields::error("42601", "syntax error");
    error.position = Some(9);
    error.hint = Some("hint".to_owned());
    out.error(&error);
    let frames = frames(out.bytes());
    let codes = frames.iter().map(|(code, _)| *code).collect::<Vec<_>>();
    assert_eq!(codes, b"RRSKZTDCI123nstvE".to_vec());
    assert_eq!(frames[0].1, 3_i32.to_be_bytes());
    assert_eq!(frames[1].1, 0_i32.to_be_bytes());
    assert_eq!(frames[2].1, b"client_encoding\0UTF8\0");
    assert_eq!(frames[4].1, b"T");
    let mut row = vec![0, 2, 0, 0, 0, 3];
    row.extend_from_slice(b"abc");
    row.extend_from_slice(&(-1_i32).to_be_bytes());
    assert_eq!(frames[6].1, row);
    let error = String::from_utf8_lossy(&frames[16].1).into_owned();
    assert!(error.contains("C42601\0"));
    assert!(error.contains("P9\0"));
    assert!(error.contains("Hhint\0"));
    assert!(error.ends_with("\0\0"));
}

#[test]
fn nul_inside_backend_strings_is_replaced() {
    let mut out = BackendWriter::new();
    out.command_complete("a\0b");
    assert_eq!(frames(out.bytes())[0].1, b"a?b\0");
}

use super::*;

#[test]
fn emits_net_tcp_stream_helpers() {
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    let tcp_listener = ValueType::Struct("TcpListener".to_string(), Vec::new());
    let tcp_stream = ValueType::Struct("TcpStream".to_string(), Vec::new());
    let result_listener_error = ValueType::Enum(
        "Result".to_string(),
        vec![tcp_listener.clone(), net_error.clone()],
    );
    let result_stream_error = ValueType::Enum(
        "Result".to_string(),
        vec![tcp_stream.clone(), net_error.clone()],
    );
    let result_string_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::String, net_error.clone()],
    );
    let result_void_error = ValueType::Enum("Result".to_string(), vec![ValueType::Void, net_error]);
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.net".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "std.net".to_string(),
                name: "NetError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "std.net".to_string(),
                name: "TcpListener".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
            StructType {
                package: "std.net".to_string(),
                name: "TcpStream".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
        ],
        enums: vec![EnumType {
            package: "std.result".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "process_stream".to_string(),
                params: vec![Parameter {
                    name: "stream".to_string(),
                    mutable: false,
                    value_type: tcp_stream,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "write_result".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::TcpStreamWriteString {
                            stream: Box::new(ValueExpr::Variable("stream".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("ping".to_string())),
                        },
                    },
                    Statement::Let {
                        name: "read_result".to_string(),
                        value_type: result_string_error,
                        initializer: ValueExpr::TcpStreamReadToString {
                            stream: Box::new(ValueExpr::Variable("stream".to_string())),
                        },
                    },
                    Statement::Expr(ValueExpr::TcpStreamClose {
                        stream: Box::new(ValueExpr::Variable("stream".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "process_listener".to_string(),
                params: vec![Parameter {
                    name: "listener".to_string(),
                    mutable: false,
                    value_type: tcp_listener,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "accepted".to_string(),
                        value_type: result_stream_error.clone(),
                        initializer: ValueExpr::TcpListenerAccept {
                            listener: Box::new(ValueExpr::Variable("listener".to_string())),
                        },
                    },
                    Statement::Expr(ValueExpr::TcpListenerClose {
                        listener: Box::new(ValueExpr::Variable("listener".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "connected".to_string(),
                        value_type: result_stream_error,
                        initializer: ValueExpr::NetConnect {
                            host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                            port: Box::new(ValueExpr::IntLiteral(7)),
                        },
                    },
                    Statement::Let {
                        name: "listening".to_string(),
                        value_type: result_listener_error,
                        initializer: ValueExpr::NetListen {
                            host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                            port: Box::new(ValueExpr::IntLiteral(7)),
                        },
                    },
                ],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_TcpListener"));
    assert!(c.contains("typedef struct nomo_struct_TcpStream"));
    assert!(c.contains("nomo_socket nomo_member_handle"));
    assert!(c.contains("static nomo_string nomo_net_error_message(void)"));
    assert!(c.contains("nomo_net_connect(nomo_string host, int64_t port)"));
    assert!(c.contains("nomo_net_listen(nomo_string host, int64_t port)"));
    assert!(c.contains("nomo_tcp_listener_accept(nomo_struct_TcpListener listener)"));
    assert!(c.contains("static void nomo_tcp_listener_close(nomo_struct_TcpListener listener)"));
    assert!(c.contains("nomo_tcp_stream_read_to_string(nomo_struct_TcpStream stream)"));
    assert!(c.contains(
        "nomo_tcp_stream_write_string(nomo_struct_TcpStream stream, nomo_string content)"
    ));
    assert!(c.contains("static void nomo_tcp_stream_close(nomo_struct_TcpStream stream)"));
    assert!(c.contains("nomo_net_connect(nomo_string_literal(\"127.0.0.1\"), 7)"));
    assert!(c.contains("nomo_net_listen(nomo_string_literal(\"127.0.0.1\"), 7)"));
    assert!(c.contains("nomo_tcp_listener_accept(nomo_listener)"));
    assert!(c.contains("nomo_tcp_listener_close(nomo_listener)"));
    assert!(c.contains("nomo_tcp_stream_write_string(nomo_stream, nomo_string_literal(\"ping\"))"));
    assert!(c.contains("nomo_tcp_stream_read_to_string(nomo_stream)"));
    assert!(c.contains("nomo_tcp_stream_close(nomo_stream)"));
}

#[test]
fn emits_net_udp_socket_helpers() {
    let net_error = ValueType::Struct("NetError".to_string(), Vec::new());
    let udp_socket = ValueType::Struct("UdpSocket".to_string(), Vec::new());
    let udp_datagram = ValueType::Struct("UdpDatagram".to_string(), Vec::new());
    let result_socket_error = ValueType::Enum(
        "Result".to_string(),
        vec![udp_socket.clone(), net_error.clone()],
    );
    let result_datagram_error = ValueType::Enum(
        "Result".to_string(),
        vec![udp_datagram.clone(), net_error.clone()],
    );
    let result_void_error = ValueType::Enum(
        "Result".to_string(),
        vec![ValueType::Void, net_error.clone()],
    );
    let program = Program {
        consts: Vec::new(),
        package: "app.main".to_string(),
        imports: vec!["std.net".to_string()],
        extern_functions: Vec::new(),
        structs: vec![
            StructType {
                package: "std.net".to_string(),
                name: "NetError".to_string(),
                type_params: Vec::new(),
                fields: vec![StructField {
                    name: "message".to_string(),
                    value_type: ValueType::String,
                }],
            },
            StructType {
                package: "std.net".to_string(),
                name: "UdpDatagram".to_string(),
                type_params: Vec::new(),
                fields: vec![
                    StructField {
                        name: "data".to_string(),
                        value_type: ValueType::String,
                    },
                    StructField {
                        name: "host".to_string(),
                        value_type: ValueType::String,
                    },
                    StructField {
                        name: "port".to_string(),
                        value_type: ValueType::Int,
                    },
                ],
            },
            StructType {
                package: "std.net".to_string(),
                name: "UdpSocket".to_string(),
                type_params: Vec::new(),
                fields: Vec::new(),
            },
        ],
        enums: vec![EnumType {
            package: "std.result".to_string(),
            name: "Result".to_string(),
            type_params: vec!["T".to_string(), "E".to_string()],
            variants: vec![
                EnumVariantType {
                    name: "Ok".to_string(),
                    payload: Some(ValueType::TypeParam("T".to_string())),
                },
                EnumVariantType {
                    name: "Err".to_string(),
                    payload: Some(ValueType::TypeParam("E".to_string())),
                },
            ],
        }],
        functions: vec![
            Function {
                package: "app.main".to_string(),
                name: "process_socket".to_string(),
                params: vec![Parameter {
                    name: "socket".to_string(),
                    mutable: false,
                    value_type: udp_socket,
                }],
                return_type: ValueType::Void,
                body: vec![
                    Statement::Let {
                        name: "packet".to_string(),
                        value_type: result_datagram_error,
                        initializer: ValueExpr::UdpSocketRecvFromString {
                            socket: Box::new(ValueExpr::Variable("socket".to_string())),
                            max_bytes: Box::new(ValueExpr::IntLiteral(1024)),
                        },
                    },
                    Statement::Let {
                        name: "sent".to_string(),
                        value_type: result_void_error,
                        initializer: ValueExpr::UdpSocketSendToString {
                            socket: Box::new(ValueExpr::Variable("socket".to_string())),
                            content: Box::new(ValueExpr::StringLiteral("pong".to_string())),
                            host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                            port: Box::new(ValueExpr::IntLiteral(7)),
                        },
                    },
                    Statement::Expr(ValueExpr::UdpSocketClose {
                        socket: Box::new(ValueExpr::Variable("socket".to_string())),
                    }),
                ],
            },
            Function {
                package: "app.main".to_string(),
                name: "main".to_string(),
                params: Vec::new(),
                return_type: ValueType::Void,
                body: vec![Statement::Let {
                    name: "bound".to_string(),
                    value_type: result_socket_error,
                    initializer: ValueExpr::NetUdpBind {
                        host: Box::new(ValueExpr::StringLiteral("127.0.0.1".to_string())),
                        port: Box::new(ValueExpr::IntLiteral(7)),
                    },
                }],
            },
        ],
    };

    let c = emit_c(&program);
    assert!(c.contains("typedef struct nomo_struct_UdpDatagram"));
    assert!(c.contains("typedef struct nomo_struct_UdpSocket"));
    assert!(c.contains("nomo_socket nomo_member_handle"));
    assert!(c.contains("nomo_net_udp_bind(nomo_string host, int64_t port)"));
    assert!(c.contains(
        "nomo_udp_socket_recv_from_string(nomo_struct_UdpSocket socket, int64_t max_bytes)"
    ));
    assert!(c.contains(
            "nomo_udp_socket_send_to_string(nomo_struct_UdpSocket socket, nomo_string content, nomo_string host, int64_t port)"
        ));
    assert!(c.contains("static void nomo_udp_socket_close(nomo_struct_UdpSocket socket)"));
    assert!(c.contains("nomo_net_udp_bind(nomo_string_literal(\"127.0.0.1\"), 7)"));
    assert!(c.contains("nomo_udp_socket_recv_from_string(nomo_socket, 1024)"));
    assert!(c.contains(
            "nomo_udp_socket_send_to_string(nomo_socket, nomo_string_literal(\"pong\"), nomo_string_literal(\"127.0.0.1\"), 7)"
        ));
    assert!(c.contains("nomo_udp_socket_close(nomo_socket)"));
}

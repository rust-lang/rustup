use native_tls::{Identity, TlsAcceptor};
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;
use std::thread;
use tokio::io::copy;
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;

pub fn proxy(server_addr: SocketAddr) -> SocketAddr {
    let addr = ([0, 0, 0, 0], 0).into();
    let listener = TcpListener::bind(&addr).expect("binding proxy address");
    let addr = listener.local_addr().expect("starting a proxy listener");

    let der = include_bytes!("cert.p12");
    let cert = Identity::from_pkcs12(der, "rustup").expect("parsing certificate");
    let tls = TlsAcceptor::builder(cert)
        .build()
        .expect("creating TLS acceptor");
    let tls = tokio_tls::TlsAcceptor::from(tls);

    thread::spawn(move || {
        let server = listener.incoming().for_each(move |socket| {
            let connection = tls
                .accept(socket)
                .map_err(|e| Error::new(ErrorKind::PermissionDenied, e))
                .and_then(move |client_socket| {
                    let server_socket = TcpStream::connect(&server_addr);
                    (future::ok(client_socket), server_socket)
                })
                .and_then(|(client_socket, server_socket)| {
                    let (client_reader, client_writer) = client_socket.split();
                    let (server_reader, server_writer) = server_socket.split();
                    let client_to_server = copy(client_reader, server_writer);
                    let server_to_client = copy(server_reader, client_writer);
                    client_to_server.join(server_to_client)
                })
                .map(|_| ())
                .map_err(|_| ());

            tokio::spawn(connection);

            Ok(())
        });

        tokio::run(server.map_err(|_| ()));
    });

    return addr;
}

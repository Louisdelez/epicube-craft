//! Contre un vrai Hub et une vraie partie relayee :
//!   PARTIE=127.0.0.1:25699#salle cargo test -p server --test relais -- --ignored
//! Un joueur passe par le relais, se presente, et entre dans la partie.
use shared::proto::{read_msg, write_msg, ClientMsg, ServerMsg, PROTOCOL_VERSION};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

#[test]
#[ignore]
fn a_player_joins_through_the_relay() {
    let address = std::env::var("PARTIE").expect("PARTIE=adresse#salle");
    let (host, room) = address.split_once('#').expect("adresse#salle");
    let mut stream = TcpStream::connect(host).unwrap();
    stream.write_all(format!("JOIN {room}\n").as_bytes()).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(15))).unwrap();
    let mut reply = [0u8; 3];
    stream.read_exact(&mut reply).expect("le relais repond");
    assert_eq!(&reply, b"OK\n");
    write_msg(&mut stream, &ClientMsg::Hello { version: PROTOCOL_VERSION, name: "Relaye".into(), view: 2, account: None }).unwrap();
    let end = Instant::now() + Duration::from_secs(10);
    let mut chunks = 0;
    let mut welcomed = false;
    while Instant::now() < end && !(welcomed && chunks >= 3) {
        match read_msg::<_, ServerMsg>(&mut stream) {
            Ok(ServerMsg::Welcome { .. }) => welcomed = true,
            Ok(ServerMsg::Chunk { .. }) => chunks += 1,
            Ok(ServerMsg::Refused { reason }) => panic!("refuse : {reason}"),
            _ => {}
        }
    }
    assert!(welcomed && chunks >= 3, "entre : {welcomed}, chunks recus : {chunks}");
}

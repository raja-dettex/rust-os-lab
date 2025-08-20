use pcap::{Active, Capture, Device};
use etherparse::{EtherType, Ethernet2Header};
pub mod arp_cache;
fn main() {
    let device = Device::list().expect("expected devices")
    .into_iter().find(|d| d.name.contains("Loopback")).expect("no valid device found for loopback");


    let mut cap = Capture::from_device(device).expect("expected capture")
        .promisc(true).immediate_mode(true).open().unwrap();

    while let Ok(packet) = cap.next_packet() {
        println!("captures ethernet frames : {packet:?}");
        if let Ok(eth) = Ethernet2Header::from_slice(packet.data) { 
            println!("source : {:?}, destination : {:?}, ether type : {:?}", eth.0.source, eth.0.destination, eth.0.ether_type);
            if eth.0.ether_type == EtherType::from(0x0806)   {
                let reply_buf = send_arp_reply( packet.data).unwrap();
                let _ = cap.sendpacket(reply_buf);
            }
        }
    }
}


fn send_arp_reply( request:  &[u8]) -> std::io::Result<[u8; 42]>{
    let arp_payload = &request[14..42]; // fixed size for ARP
    let sender_mac = &arp_payload[8..14];
    let sender_ip = &arp_payload[14..18];
    let target_ip = &arp_payload[24..28];
    

    // Build reply frame in a fixed buffer
    let mut buf = [0u8; 42];
    
    // Ethernet header
    buf[0..6].copy_from_slice(sender_mac);     // dest = requester
    buf[6..12].copy_from_slice(&[0x02,0x00,0x00,0x00,0x00,0x01]); // our MAC (example)
    buf[12..14].copy_from_slice(&[0x08, 0x06]); // Ethertype = ARP

    // ARP header
    buf[14..16].copy_from_slice(&[0x00, 0x01]); // HW type Ethernet
    buf[16..18].copy_from_slice(&[0x08, 0x00]); // Proto IPv4
    buf[18] = 6;  // HW size
    buf[19] = 4;  // Proto size
    buf[20..22].copy_from_slice(&[0x00, 0x02]); // Opcode = reply
    buf[22..28].copy_from_slice(&[0x02,0x00,0x00,0x00,0x00,0x01]); // sender MAC
    buf[28..32].copy_from_slice(target_ip);      // sender IP = target IP of request
    buf[32..38].copy_from_slice(sender_mac);     // target MAC = requester MAC
    buf[38..42].copy_from_slice(sender_ip);      // target IP = requester IP
    Ok(buf)

}

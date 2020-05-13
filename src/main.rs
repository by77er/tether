use igd;
use clap;

use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let args = clap::App::new("tether")
        .version("0.1.0")
        .arg(clap::Arg::with_name("internal-ip")
            .long("internal-ip")
            .short("i")
            .help("Which internal host should receive forwarded packets")
            .takes_value(true)
            .required(true))
        .arg(clap::Arg::with_name("internal-port")
            .long("internal-port")
            .short("p")
            .help("Port on the internal host to which packets should be forwarded")
            .takes_value(true)
            .required(true))
        .arg(clap::Arg::with_name("external-port")
            .long("external-port")
            .short("e")
            .help("Which port the gateway should listen on for connections")
            .takes_value(true)
            .required(true))
        .arg(clap::Arg::with_name("type")
            .long("type")
            .short("t")
            .help("TCP or UDP (default: TCP)")
            .takes_value(true)
            .required(false))
        .arg(clap::Arg::with_name("description")
            .long("description")
            .short("d")
            .help("A short description for this port-forward")
            .takes_value(true)
            .required(false))
        .arg(clap::Arg::with_name("lease-length")
            .long("lease-length")
            .short("l")
            .help("The duration of the mapping (default: forever)")
            .takes_value(true)
            .required(false))
        .get_matches();
    
    // Gathering arguments
    let proto = args.value_of("type").unwrap_or("TCP").to_ascii_uppercase();
    let lease = u32::from_str_radix(args.value_of("lease-length").unwrap_or("0"), 10)?;
    let external_port = u16::from_str_radix(args.value_of("external-port").unwrap(), 10)?;
    let internal_port = args.value_of("internal-port").unwrap();
    let internal_ip = args.value_of("internal-ip").unwrap();
    let description = args.value_of("description").unwrap_or("");
    // Creating a socket address
    let mut host_string = String::new();
    host_string.push_str(internal_ip);
    host_string.push(':');
    host_string.push_str(internal_port);
    let socket: std::net::SocketAddrV4 = host_string.parse()?;
    // Determining protocol
    let protocol = match proto.as_ref() {
        "TCP" => igd::PortMappingProtocol::TCP,
        "UDP" => igd::PortMappingProtocol::UDP,
        _ => return Err("Protocol type must be 'TCP' or 'UDP'".into())
    };

    println!("socket: {:?}", socket);
    // Finds the gateway device to use for port forwarding
    let gateway = igd::search_gateway(Default::default())?;
    println!("Found gateway:");
    // Prints the gateway device's IP
    let external = gateway.get_external_ip()?;
    println!("  External address: {:?}", external);
    println!("Adding configuration {} {}:{} -> {:?}", proto, external, external_port, socket);

    gateway.add_port(protocol, external_port, socket, lease, description)?;
    Ok(())
}



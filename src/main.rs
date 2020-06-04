use clap::{load_yaml, App, ArgMatches};
use igd::{
    Gateway, GetGenericPortMappingEntryError, PortMappingEntry, PortMappingProtocol, SearchOptions,
};

use PortMappingProtocol::*;

use core::time::Duration;
use net::SocketAddrV4;
use std::net;
use std::process;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let args = App::from_yaml(yaml).get_matches();

    // Dispatch the appropriate subroutine
    match args.subcommand() {
        ("view", Some(matches)) => view_branch(&matches),
        ("add", Some(matches)) => add_branch(&matches),
        ("remove", Some(matches)) => remove_branch(&matches),
        _ => fail(args.usage()),
    }
}

// Given a timeout, finds the gateway device
fn get_gateway(args: &ArgMatches) -> igd::Gateway {
    let timeout: u64 = args
        .value_of("timeout")
        .unwrap_or("60")
        .parse()
        .unwrap_or_else(|_e| fail("Invalid timeout. Please choose an integer > 0."));

    let options = SearchOptions {
        timeout: Some(Duration::from_secs(timeout)),
        ..Default::default()
    };
    // Scan the network for the gateway
    igd::search_gateway(options).unwrap_or_else(|_e| fail("Failed to find UPNP-enabled gateway."))
}

// Retrieves the mappings from the router
fn get_current_mappings(gateway: &Gateway) -> Vec<PortMappingEntry> {
    let mut ports: Vec<PortMappingEntry> = Vec::new();

    for index in 0..u32::MAX {
        match gateway.get_generic_port_mapping_entry(index) {
            Ok(entry) => {
                ports.push(entry);
            }
            Err(error) => match error {
                // Hit the end of the host list - stop printing and exit.
                GetGenericPortMappingEntryError::SpecifiedArrayIndexInvalid => break,
                // Permission issue - print an error and exit
                e => fail(&format!("Error getting port mapping: {}", e)),
            },
        }
    }
    // Return mappings
    ports
}

// Supports the "view" subcommand functionality - "print" argument allows it to be used by other fns
fn view_branch(matches: &ArgMatches) {
    let gateway = get_gateway(&matches);
    if matches.is_present("print_ip") {
        let ip = &gateway
            .get_external_ip()
            .unwrap_or_else(|_e| fail("Failed to get external IP address."));
        println!("Internal IP: {} | External IP: {}", &gateway.addr.ip(), ip);
    }

    let ports = get_current_mappings(&gateway);

    println!(
        "{: <15} || {: <15} || {: <25} // Description",
        "Remote Host", "External Port", "Internal Host",
    );

    for port in ports {
        let internal_combined = format!(
            "{}/{}@{}",
            port.internal_port, port.protocol, port.internal_client
        );
        let external_combined = format!("{}/{}", port.external_port, port.protocol);
        println!(
            "{: <15} -> {: <15} -> {: <25} // {}",
            port.remote_host, external_combined, internal_combined, port.port_mapping_description
        );
    }
}

// Supports the "add" subcommand functionality
fn add_branch(matches: &ArgMatches) {
    // So many arguments to match...
    let external_port: u16 = matches
        .value_of("external_port")
        .unwrap()
        .parse()
        .unwrap_or_else(|_e| {
            fail("Invalid external port. Please choose an integer from 0 to 65535.")
        });

    let internal_port: u16 = matches
        .value_of("internal_port")
        .unwrap()
        .parse()
        .unwrap_or_else(|_e| {
            fail("Invalid external port. Please choose an integer from 0 to 65535.")
        });

    let internal_host: net::Ipv4Addr = matches
        .value_of("internal_host")
        .unwrap()
        .parse()
        .unwrap_or_else(|_e| {
            fail("Invalid internal host address. Please specify an IPv4 address.")
        });

    let lease_duration: u32 = match matches.value_of("lease_duration") {
        Some(d) => d
            .parse()
            .unwrap_or_else(|_e| fail("Invalid duration. Choose an integer >= 0")),
        None => 0,
    };

    let description: &str = matches.value_of("description").unwrap_or("");

    let proto_string = match matches.value_of("protocol") {
        Some(s) => s,
        None => fail("Please specify a protocol. TCP, UDP, and BOTH are valid."),
    };

    match proto_string.to_ascii_uppercase().as_str() {
        "UDP" => {
            add_port(
                &mut get_gateway(&matches),
                UDP,
                external_port,
                SocketAddrV4::new(internal_host, internal_port),
                lease_duration,
                description,
            );
        }
        "TCP" => {
            add_port(
                &mut get_gateway(&matches),
                TCP,
                external_port,
                SocketAddrV4::new(internal_host, internal_port),
                lease_duration,
                description,
            );
        }
        "BOTH" => {
            let mut gateway = get_gateway(&matches);
            add_port(
                &mut gateway,
                UDP,
                external_port,
                SocketAddrV4::new(internal_host, internal_port),
                lease_duration,
                description,
            );
            add_port(
                &mut gateway,
                TCP,
                external_port,
                SocketAddrV4::new(internal_host, internal_port),
                lease_duration,
                description,
            );
        }
        _ => fail("Invalid protocol. TCP, UDP, and BOTH are valid."),
    }
}

// Adds the port mapping
fn add_port(
    gateway: &mut Gateway,
    protocol: PortMappingProtocol,
    external_port: u16,
    local_addr: net::SocketAddrV4,
    lease_duration: u32,
    description: &str,
) {
    match gateway.add_port(
        protocol,
        external_port,
        local_addr,
        lease_duration,
        description,
    ) {
        // Hooray, port forwarded
        Ok(()) => println!(
            "Added mapping: {} ext:{} -> {}",
            protocol, external_port, local_addr
        ),
        // Something went wrong
        Err(e) => fail(&format!("Error adding port mapping: {}", e)),
    }
}

// Supports the "remove" subcommand functionality
fn remove_branch(matches: &ArgMatches) {
    // Clear case
    if matches.is_present("clear_all") {
        let mut gateway = get_gateway(&matches);
        let entries = get_current_mappings(&gateway);
        // Notify when nothing happens
        if entries.is_empty() {
            println!("No mappings to delete.");
        }
        for entry in entries {
            remove_port(&mut gateway, entry.protocol, entry.external_port);
        }
        // don't try to remove anything else
        return;
    }

    // Normal case
    let external_port: u16 = matches
        .value_of("external_port")
        .unwrap_or_else(|| fail("Please specify an external port."))
        .parse()
        .unwrap_or_else(|_e| {
            fail("Invalid external port. Please choose an integer from 0 to 65535.")
        });

    // Validate protocol and remove (BOTH will still try to complete the other request if
    // the first one fails)
    let proto_string = match matches.value_of("protocol") {
        Some(s) => s,
        None => fail("Please specify a protocol. TCP, UDP, and BOTH are valid."),
    };

    match proto_string.to_ascii_uppercase().as_str() {
        "UDP" => {
            remove_port(&mut get_gateway(&matches), UDP, external_port);
        }
        "TCP" => {
            remove_port(&mut get_gateway(&matches), TCP, external_port);
        }
        "BOTH" => {
            let mut gateway = get_gateway(&matches);
            remove_port(&mut gateway, UDP, external_port);
            remove_port(&mut gateway, TCP, external_port);
        }
        _ => fail("Invalid protocol. TCP, UDP, and BOTH are valid."),
    }
}

// removes a port mapping
fn remove_port(gateway: &mut Gateway, protocol: PortMappingProtocol, external_port: u16) {
    match gateway.remove_port(protocol, external_port) {
        // Removed, yay
        Ok(()) => println!("Removed {}/{}", external_port, protocol),
        // Unauthorized.
        Err(e) => fail(&format!("Error removing port mapping: {}", e)),
    }
}

// Prints an error message and exits
fn fail(message: &str) -> ! {
    eprintln!("{}", message);
    process::exit(1);
}

use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    match args[1].as_str() {
        "registry" => handle_registry(&args[2..]),
        "status" => handle_status(),
        "help" | "--help" | "-h" => print_usage(),
        _ => {
            eprintln!("Unknown command: {}", args[1]);
            print_usage();
            process::exit(1);
        }
    }
}

fn handle_registry(args: &[String]) {
    if args.is_empty() {
        eprintln!("registry command requires a subcommand: start, stop, status, clean");
        process::exit(1);
    }

    match args[0].as_str() {
        "start" => registry_start(),
        "stop" => registry_stop(),
        "status" => registry_status(),
        "clean" => registry_clean(),
        _ => {
            eprintln!("Unknown registry subcommand: {}", args[0]);
            process::exit(1);
        }
    }
}

fn registry_start() {
    println!("Starting local OCI registry...");
    // TODO: Implement registry start
    println!("Registry will be available at localhost:5000");
}

fn registry_stop() {
    println!("Stopping local OCI registry...");
    // TODO: Implement registry stop
}

fn registry_status() {
    println!("Checking registry status...");
    // TODO: Implement registry status check
}

fn registry_clean() {
    println!("Cleaning registry data...");
    // TODO: Implement registry clean
}

fn handle_status() {
    println!("Forest Status:");
    println!("  Registry: Not implemented");
    println!("  Built kits: Not implemented");
    println!("  Built variants: Not implemented");
}

fn print_usage() {
    println!("Forest - Bottlerocket development orchestration tool");
    println!();
    println!("USAGE:");
    println!("    forest <COMMAND>");
    println!();
    println!("COMMANDS:");
    println!("    registry    Manage local OCI registry");
    println!("      start       Start the local registry");
    println!("      stop        Stop the local registry");
    println!("      status      Check registry status");
    println!("      clean       Remove registry data");
    println!("    status      Show forest development status");
    println!("    help        Print this help message");
}

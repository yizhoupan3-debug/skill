use clap::Parser;

fn main() -> Result<(), String> {
    let args = router_rs::cli::Cli::parse();
    router_rs::cli::run(&args)
}

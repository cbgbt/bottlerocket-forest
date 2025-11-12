use forester::cli;

#[snafu::report]
fn main() -> Result<(), cli::CliError> {
    cli::run()
}

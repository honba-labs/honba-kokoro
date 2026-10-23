//! Golden Test Runner for Broker Contract Note Validation
//!
//! This binary runs the golden test suite against broker contract notes.
//! Usage: cargo run --bench golden_tests -- [OPTIONS]

use clap::Parser;
use honba_kokoro_ledger::golden_tests::{GoldenTestRunner, generate_sample_csv};
use rust_decimal::Decimal;
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "golden_tests")]
#[command(about = "Golden Test Runner for Broker Contract Note Validation")]
struct Args {
    /// Output results as JSON
    #[arg(long)]
    json: bool,

    /// Tolerance in paise (default: 1)
    #[arg(long, default_value = "1")]
    tolerance: i64,

    /// Filter by broker name
    #[arg(long)]
    broker: Option<String>,

    /// Input CSV/JSON file path
    #[arg(long, short)]
    input: Option<PathBuf>,

    /// Output JSON results to file
    #[arg(long, short)]
    output: Option<PathBuf>,

    /// Generate N sample trades for testing
    #[arg(long)]
    generate: Option<usize>,

    /// Run in verbose mode
    #[arg(long, short)]
    verbose: bool,

    /// Internal flag for cargo bench
    #[arg(long, hide = true)]
    bench: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // If generate flag is set, generate sample CSV and exit
    if let Some(count) = args.generate {
        let broker = args.broker.as_deref().unwrap_or("zerodha");
        let csv = generate_sample_csv(broker, count);

        if let Some(output) = args.output {
            std::fs::write(&output, csv)?;
            println!("Generated {} sample trades to {}", count, output.display());
        } else {
            println!("{}", csv);
        }
        return Ok(());
    }

    let mut runner = GoldenTestRunner::new().with_tolerance(Decimal::from(args.tolerance));

    // Load trades from input file if provided
    if let Some(ref input_path) = args.input {
        let ext = input_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");

        if ext == "csv" {
            let _broker = args.broker.as_deref().unwrap_or("zerodha");
            let mut runner_with_trades =
                GoldenTestRunner::new().with_tolerance(rust_decimal::Decimal::from(args.tolerance));
            runner_with_trades =
                runner_with_trades.load_from_csv(&input_path.to_string_lossy(), "zerodha")?;
            runner = runner_with_trades;
        } else if ext == "json" {
            let mut runner_with_trades =
                GoldenTestRunner::new().with_tolerance(rust_decimal::Decimal::from(args.tolerance));
            runner_with_trades =
                runner_with_trades.load_from_json(&input_path.to_string_lossy())?;
            runner = runner_with_trades;
        } else {
            eprintln!("Unsupported file format. Use .csv or .json");
            std::process::exit(1);
        }
    }

    // If broker filter is set, we'd need to filter (not implemented in runner yet)
    // For now, run with built-in test trades if no input provided
    if args.input.is_none() {
        // Run with built-in test trades from the test module
        let mut runner_with_trades =
            GoldenTestRunner::new().with_tolerance(rust_decimal::Decimal::from(args.tolerance));

        // Add the built-in test trades
        runner_with_trades = runner_with_trades
            .add_trade(
                honba_kokoro_ledger::golden_tests::test_zerodha_contract_note_delivery_buy_trade(),
            )
            .add_trade(
                honba_kokoro_ledger::golden_tests::test_zerodha_contract_note_intraday_sell_trade(),
            )
            .add_trade(
                honba_kokoro_ledger::golden_tests::test_upstox_contract_note_delivery_buy_trade(),
            )
            .add_trade(
                honba_kokoro_ledger::golden_tests::test_upstox_contract_note_option_buy_trade(),
            )
            .add_trade(
                honba_kokoro_ledger::golden_tests::test_total_charges_match_broker_note_trade(),
            );

        runner = runner_with_trades;
    }

    // Run the tests
    let report = runner.run();

    // Print summary
    if args.verbose || !args.json {
        report.print_summary();
    }

    // Output JSON if requested
    if args.json || args.output.is_some() {
        let json = report.to_json()?;

        if let Some(output) = args.output {
            fs::write(output, json)?;
            println!("Results written to file");
        } else if args.json {
            println!("{}", json);
        }
    }

    // Exit with error code if any tests failed
    if report.failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

//! Command-line front end: run phase-1 procedures on a data file (.sav, .csv, .txt, .xlsx).
use dataset::{Dataset, VarId};
use output_model::{OutputItem, render_html, render_text};
use procedures::explore::{ExploreOptions, run_explore};
use procedures::npar_ks::{NparKsOptions, run_npar_ks};
use procedures::npar_runs::{RunsOptions, run_runs};
use procedures::oneway::{OneWayOptions, run_oneway};
use procedures::regression::{RegressionOptions, SaveFlags, run_regression};
use procedures::ttest_ind::{IndependentOptions, run_independent};
use procedures::ttest_one::{OneSampleOptions, run_one_sample};
use procedures::ttest_paired::{PairedOptions, run_paired};
use std::collections::HashMap;
use std::process::ExitCode;

const USAGE: &str = "\
statik-cli - run Statik procedures on a CSV file

USAGE:
  statik-cli regression   <file> --dep y --indep x1,x2,x3 [--html out.html]
  statik-cli ttest-ind    <file> --var v --group g --groups 1,2 [--html out.html]
  statik-cli ttest-paired <file> --pair a,b [--html out.html]
  statik-cli ttest-one    <file> --var v --value 50 [--html out.html]
  statik-cli explore      <file> --var v [--html out.html]
  statik-cli ks           <file> --var v [--html out.html]
  statik-cli runs         <file> --var v [--html out.html]
  statik-cli oneway       <file> --dep y --factor g [--html out.html]
";

enum CliError {
    Usage(String),
    Io(String),
}

type Res<T> = Result<T, CliError>;

fn usage<T>(m: impl Into<String>) -> Res<T> {
    Err(CliError::Usage(m.into()))
}

struct Args {
    file: String,
    opts: HashMap<String, String>,
}

fn parse_args(rest: &[String]) -> Res<Args> {
    let mut it = rest.iter();
    let file = match it.next() {
        Some(c) if !c.starts_with("--") => c.clone(),
        _ => return usage("missing <file> argument"),
    };
    let mut opts = HashMap::new();
    while let Some(k) = it.next() {
        let Some(name) = k.strip_prefix("--") else {
            return usage(format!("unexpected argument: {k}"));
        };
        let Some(v) = it.next() else {
            return usage(format!("option --{name} needs a value"));
        };
        opts.insert(name.to_string(), v.clone());
    }
    Ok(Args { file, opts })
}

impl Args {
    fn req(&self, k: &str) -> Res<&str> {
        match self.opts.get(k) {
            Some(v) => Ok(v),
            None => usage(format!("missing required option --{k}")),
        }
    }
}

fn var(ds: &Dataset, name: &str) -> Res<VarId> {
    match ds.find(name) {
        Some(id) => Ok(id),
        None => usage(format!("unknown variable: {name}")),
    }
}

fn vars(ds: &Dataset, list: &str) -> Res<Vec<VarId>> {
    list.split(',').map(|n| var(ds, n.trim())).collect()
}

fn number(s: &str) -> Res<f64> {
    match s.trim().parse::<f64>() {
        Ok(v) if v.is_finite() => Ok(v),
        _ => usage(format!("bad number: {s}")),
    }
}

fn run(args: &[String]) -> Res<()> {
    let Some(cmd) = args.first() else {
        eprint!("{USAGE}");
        return usage("no subcommand given");
    };
    if cmd == "--help" || cmd == "-h" || cmd == "help" {
        print!("{USAGE}");
        return Ok(());
    }
    const KNOWN: [&str; 8] =
        ["regression", "ttest-ind", "ttest-paired", "ttest-one", "explore", "ks", "runs", "oneway"];
    if !KNOWN.contains(&cmd.as_str()) {
        return usage(format!("unknown subcommand: {cmd}"));
    }
    let a = parse_args(&args[1..])?;
    let (mut ds, report) = sav_io::open_any(std::path::Path::new(&a.file))
        .map_err(|e| CliError::Io(format!("cannot read {}: {e}", a.file)))?;
    for note in &report.notes {
        eprintln!("Note: {note}");
    }
    let items: Vec<OutputItem> = match cmd.as_str() {
        "regression" => {
            let dependent = var(&ds, a.req("dep")?)?;
            let independents = vars(&ds, a.req("indep")?)?;
            let o = RegressionOptions {
                dependent,
                independents,
                collinearity: true,
                durbin_watson: true,
                plot_hist: true,
                plot_pp: true,
                plot_zpred_sresid: true,
                save: SaveFlags::default(),
            };
            run_regression(&mut ds, &o)
        }
        "ttest-ind" => {
            let test_vars = vars(&ds, a.req("var")?)?;
            let grouping = var(&ds, a.req("group")?)?;
            let g = a.req("groups")?;
            let parts: Vec<&str> = g.split(',').collect();
            if parts.len() != 2 {
                return usage("--groups needs exactly two values, e.g. 1,2");
            }
            let o = IndependentOptions {
                test_vars,
                grouping,
                group1: number(parts[0])?,
                group2: number(parts[1])?,
                conf: 0.95,
            };
            run_independent(&ds, &o)
        }
        "ttest-paired" => {
            let p = a.req("pair")?;
            let parts: Vec<&str> = p.split(',').collect();
            if parts.len() != 2 {
                return usage("--pair needs exactly two variables, e.g. a,b");
            }
            let o = PairedOptions { pairs: vec![(var(&ds, parts[0].trim())?, var(&ds, parts[1].trim())?)], conf: 0.95 };
            run_paired(&ds, &o)
        }
        "ttest-one" => {
            let o = OneSampleOptions {
                vars: vars(&ds, a.req("var")?)?,
                test_value: number(a.req("value")?)?,
                conf: 0.95,
            };
            run_one_sample(&ds, &o)
        }
        "explore" => {
            let o = ExploreOptions { dependents: vars(&ds, a.req("var")?)?, normality: true, qq_plot: true };
            run_explore(&ds, &o)
        }
        "ks" => run_npar_ks(&ds, &NparKsOptions { vars: vars(&ds, a.req("var")?)? }),
        "runs" => run_runs(&ds, &RunsOptions { vars: vars(&ds, a.req("var")?)? }),
        "oneway" => {
            let o = OneWayOptions {
                dependent: var(&ds, a.req("dep")?)?,
                factor: var(&ds, a.req("factor")?)?,
                descriptives: true,
                homogeneity: true,
            };
            run_oneway(&ds, &o)
        }
        _ => unreachable!(),
    };
    print!("{}", render_text(&items));
    if let Some(path) = a.opts.get("html") {
        let html = format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><title>Statik Output</title></head><body>{}</body></html>",
            render_html(&items)
        );
        std::fs::write(path, html).map_err(|e| CliError::Io(format!("cannot write {path}: {e}")))?;
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(CliError::Usage(m)) => {
            eprintln!("error: {m}");
            ExitCode::from(2)
        }
        Err(CliError::Io(m)) => {
            eprintln!("error: {m}");
            ExitCode::from(1)
        }
    }
}

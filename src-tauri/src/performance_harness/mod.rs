#[cfg_attr(not(test), allow(dead_code))]
mod compare;
#[cfg_attr(not(test), allow(dead_code))]
mod dataset;
#[cfg_attr(not(test), allow(dead_code))]
mod metrics;
#[cfg_attr(not(test), allow(dead_code))]
mod owned_temp;
#[cfg_attr(not(test), allow(dead_code))]
mod protocol;
#[cfg_attr(not(test), allow(dead_code))]
mod scenarios;

pub(crate) fn run_if_requested(args: &[String]) -> Option<anyhow::Result<()>> {
    let marker = args.iter().position(|arg| arg == "--performance-harness")?;
    let command = args.get(marker + 1).map(String::as_str).unwrap_or("");
    Some(match command {
        "self-test-entry" => {
            println!("PureWall performance harness entry OK");
            Ok(())
        }
        other => Err(anyhow::anyhow!(
            "unknown performance harness command: {other}"
        )),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_arguments_do_not_enter_the_harness() {
        let args = vec!["purewall.exe".into(), "--action".into(), "next".into()];
        assert!(run_if_requested(&args).is_none());
    }

    #[test]
    fn explicit_marker_runs_entry_self_test() {
        let args = vec![
            "purewall.exe".into(),
            "--performance-harness".into(),
            "self-test-entry".into(),
        ];
        assert!(run_if_requested(&args).expect("marker consumed").is_ok());
    }

    #[test]
    fn unknown_command_is_controlled() {
        let args = vec![
            "purewall.exe".into(),
            "--performance-harness".into(),
            "unknown".into(),
        ];
        let error = run_if_requested(&args)
            .expect("marker consumed")
            .expect_err("must fail");
        assert_eq!(
            error.to_string(),
            "unknown performance harness command: unknown"
        );
    }
}

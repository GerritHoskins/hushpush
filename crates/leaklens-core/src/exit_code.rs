/// Stable process outcomes for callers and Git hooks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExitCode {
    NoBlockingFindings = 0,
    BlockingFindings = 1,
    UsageOrConfigError = 2,
    IncompleteCoverage = 3,
    InternalError = 4,
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(value: ExitCode) -> Self {
        Self::from(value as u8)
    }
}

#[cfg(test)]
mod tests {
    use super::ExitCode;

    #[test]
    fn process_codes_match_the_contract() {
        for (outcome, code) in [
            (ExitCode::NoBlockingFindings, 0),
            (ExitCode::BlockingFindings, 1),
            (ExitCode::UsageOrConfigError, 2),
            (ExitCode::IncompleteCoverage, 3),
            (ExitCode::InternalError, 4),
        ] {
            assert_eq!(outcome as u8, code);
            assert_eq!(
                std::process::ExitCode::from(outcome),
                std::process::ExitCode::from(code)
            );
        }
    }
}

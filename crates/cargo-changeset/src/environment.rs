use std::io::IsTerminal;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum NonInteractiveReason {
    ExplicitDisable,
    CiDetected { env_var: String },
    NoTerminal,
}

pub(crate) fn is_interactive() -> bool {
    non_interactive_reason().is_none()
}

pub(crate) fn non_interactive_reason() -> Option<NonInteractiveReason> {
    if std::env::var("CARGO_CHANGESET_NO_TTY").is_ok() {
        return Some(NonInteractiveReason::ExplicitDisable);
    }

    if std::env::var("CARGO_CHANGESET_FORCE_TTY").is_ok() {
        return None;
    }

    if let Some(env_var) = detect_ci_env_var() {
        return Some(NonInteractiveReason::CiDetected { env_var });
    }

    if !std::io::stdin().is_terminal() {
        return Some(NonInteractiveReason::NoTerminal);
    }

    None
}

fn detect_ci_env_var() -> Option<String> {
    const CI_ENV_VARS: &[&str] = &[
        "CI",
        "GITHUB_ACTIONS",
        "GITLAB_CI",
        "CIRCLECI",
        "TRAVIS",
        "JENKINS_URL",
        "BUILDKITE",
        "TF_BUILD",
    ];

    for var in CI_ENV_VARS {
        if std::env::var(var).is_ok() {
            return Some((*var).to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_vars(
        extra: &[(&'static str, Option<&'static str>)],
    ) -> Vec<(&'static str, Option<&'static str>)> {
        let mut vars: Vec<(&str, Option<&str>)> = [
            "CI",
            "GITHUB_ACTIONS",
            "GITLAB_CI",
            "CIRCLECI",
            "TRAVIS",
            "JENKINS_URL",
            "BUILDKITE",
            "TF_BUILD",
            "CARGO_CHANGESET_NO_TTY",
            "CARGO_CHANGESET_FORCE_TTY",
        ]
        .map(|var| (var, None))
        .to_vec();
        vars.extend_from_slice(extra);
        vars
    }

    mod detect_ci_env_var {
        use super::*;

        #[test]
        fn returns_none_when_no_ci_vars_set() {
            temp_env::with_vars(env_vars(&[]), || {
                assert!(detect_ci_env_var().is_none());
            });
        }

        #[test]
        fn detects_ci() {
            temp_env::with_vars(env_vars(&[("CI", Some("true"))]), || {
                assert_eq!(detect_ci_env_var(), Some("CI".to_string()));
            });
        }

        #[test]
        fn detects_github_actions() {
            temp_env::with_vars(env_vars(&[("GITHUB_ACTIONS", Some("true"))]), || {
                assert_eq!(detect_ci_env_var(), Some("GITHUB_ACTIONS".to_string()));
            });
        }

        #[test]
        fn detects_gitlab_ci() {
            temp_env::with_vars(env_vars(&[("GITLAB_CI", Some("true"))]), || {
                assert_eq!(detect_ci_env_var(), Some("GITLAB_CI".to_string()));
            });
        }

        #[test]
        fn detects_circleci() {
            temp_env::with_vars(env_vars(&[("CIRCLECI", Some("true"))]), || {
                assert_eq!(detect_ci_env_var(), Some("CIRCLECI".to_string()));
            });
        }

        #[test]
        fn detects_travis() {
            temp_env::with_vars(env_vars(&[("TRAVIS", Some("true"))]), || {
                assert_eq!(detect_ci_env_var(), Some("TRAVIS".to_string()));
            });
        }

        #[test]
        fn detects_jenkins() {
            temp_env::with_vars(
                env_vars(&[("JENKINS_URL", Some("http://jenkins.local"))]),
                || {
                    assert_eq!(detect_ci_env_var(), Some("JENKINS_URL".to_string()));
                },
            );
        }

        #[test]
        fn detects_buildkite() {
            temp_env::with_vars(env_vars(&[("BUILDKITE", Some("true"))]), || {
                assert_eq!(detect_ci_env_var(), Some("BUILDKITE".to_string()));
            });
        }

        #[test]
        fn detects_azure_devops() {
            temp_env::with_vars(env_vars(&[("TF_BUILD", Some("True"))]), || {
                assert_eq!(detect_ci_env_var(), Some("TF_BUILD".to_string()));
            });
        }
    }

    mod non_interactive_reason_tests {
        use super::*;

        #[test]
        fn no_tty_takes_highest_priority() {
            temp_env::with_vars(
                env_vars(&[
                    ("CARGO_CHANGESET_NO_TTY", Some("1")),
                    ("CARGO_CHANGESET_FORCE_TTY", Some("1")),
                    ("CI", Some("true")),
                ]),
                || {
                    assert_eq!(
                        non_interactive_reason(),
                        Some(NonInteractiveReason::ExplicitDisable)
                    );
                },
            );
        }

        #[test]
        fn force_tty_takes_priority_over_ci_detection() {
            temp_env::with_vars(
                env_vars(&[
                    ("CI", Some("true")),
                    ("CARGO_CHANGESET_FORCE_TTY", Some("1")),
                ]),
                || {
                    assert!(non_interactive_reason().is_none());
                },
            );
        }

        #[test]
        fn force_tty_allows_interactivity_when_no_ci() {
            temp_env::with_vars(
                env_vars(&[("CARGO_CHANGESET_FORCE_TTY", Some("1"))]),
                || {
                    assert!(non_interactive_reason().is_none());
                },
            );
        }

        #[test]
        fn explicit_disable_returns_correct_reason() {
            temp_env::with_vars(env_vars(&[("CARGO_CHANGESET_NO_TTY", Some("1"))]), || {
                assert_eq!(
                    non_interactive_reason(),
                    Some(NonInteractiveReason::ExplicitDisable)
                );
            });
        }

        #[test]
        fn ci_detection_returns_correct_env_var() {
            temp_env::with_vars(env_vars(&[("GITHUB_ACTIONS", Some("true"))]), || {
                assert_eq!(
                    non_interactive_reason(),
                    Some(NonInteractiveReason::CiDetected {
                        env_var: "GITHUB_ACTIONS".to_string()
                    })
                );
            });
        }
    }

    mod is_interactive_tests {
        use super::*;

        #[test]
        fn returns_false_when_no_tty_set() {
            temp_env::with_vars(env_vars(&[("CARGO_CHANGESET_NO_TTY", Some("1"))]), || {
                assert!(!is_interactive());
            });
        }

        #[test]
        fn returns_false_when_ci_detected() {
            temp_env::with_vars(env_vars(&[("CI", Some("true"))]), || {
                assert!(!is_interactive());
            });
        }

        #[test]
        fn returns_true_when_force_tty_and_no_ci() {
            temp_env::with_vars(
                env_vars(&[("CARGO_CHANGESET_FORCE_TTY", Some("1"))]),
                || {
                    assert!(is_interactive());
                },
            );
        }

        #[test]
        fn returns_true_when_force_tty_overrides_ci() {
            temp_env::with_vars(
                env_vars(&[
                    ("CI", Some("true")),
                    ("CARGO_CHANGESET_FORCE_TTY", Some("1")),
                ]),
                || {
                    assert!(is_interactive());
                },
            );
        }
    }
}

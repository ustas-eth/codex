//! Forward explicit programs while leaving entitlement and model policy to the server.

use codex_api::AccessPrograms;
use codex_config::CyberAccessProgramPreference;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::openai_models::ModelAccessPrograms;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::turn_input::CyberAccessProgram;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ApiKeyCyberAccessPrograms {
    UnsupportedProvider,
    Disabled,
    Enabled,
}

impl ApiKeyCyberAccessPrograms {
    pub(crate) fn from_config(config: &crate::config::Config) -> Self {
        if config.model_provider_id != OPENAI_PROVIDER_ID {
            Self::UnsupportedProvider
        } else if config.features.enabled(Feature::ApiKeyCyberAccessPrograms) {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

pub(crate) fn for_provider(
    provider_id: &str,
    program: Option<CyberAccessProgram>,
) -> Option<CyberAccessProgram> {
    program.filter(|_| provider_id == OPENAI_PROVIDER_ID)
}

/// Resolve subscription defaults without changing upstream's explicit API-key policy.
/// Explicit turns and saved thread choices win over configuration and inheritance.
pub(crate) fn for_turn(
    config: &crate::config::Config,
    model: &ModelInfo,
    auth: Option<&CodexAuth>,
    explicit: Option<CyberAccessProgram>,
    daybreak_preference: Option<bool>,
    inherited: Option<CyberAccessProgram>,
) -> Option<CyberAccessProgram> {
    let thread_program = auth
        .filter(|auth| {
            auth.is_chatgpt_auth()
                || (auth.is_api_key_auth()
                    && config.features.enabled(Feature::ApiKeyCyberAccessPrograms))
        })
        .and(daybreak_preference)
        .map(|enabled| saved_thread_program(model, enabled));
    let preference = auth.filter(|auth| auth.is_chatgpt_auth()).and_then(|_| {
        config
            .cyber_access_program_by_model
            .get(&model.slug)
            .copied()
            .or(config.cyber_access_program)
    });
    for_provider(
        &config.model_provider_id,
        explicit.or(thread_program).or(match preference {
            Some(CyberAccessProgramPreference::Auto) => None,
            Some(CyberAccessProgramPreference::Standard) => Some(CyberAccessProgram::Standard),
            Some(CyberAccessProgramPreference::DaybreakBlue) => {
                Some(CyberAccessProgram::DaybreakBlue)
            }
            Some(CyberAccessProgramPreference::DaybreakRed) => {
                Some(CyberAccessProgram::DaybreakRed)
            }
            None => inherited,
        }),
    )
}

fn saved_thread_program(model: &ModelInfo, enabled: bool) -> CyberAccessProgram {
    if !enabled {
        return CyberAccessProgram::Standard;
    }
    // Leave unsupported explicit requests to the backend instead of silently downgrading.
    model
        .available_access_programs
        .as_ref()
        .and_then(ModelAccessPrograms::daybreak)
        .unwrap_or(CyberAccessProgram::DaybreakBlue)
}

pub(crate) fn for_auth(
    auth: Option<&CodexAuth>,
    program: Option<CyberAccessProgram>,
    policy: ApiKeyCyberAccessPrograms,
) -> Result<Option<AccessPrograms>> {
    let Some(program) = program else {
        return Ok(None);
    };
    let Some(auth) = auth else {
        return Ok(None);
    };
    if auth.is_chatgpt_auth() {
        return Ok(Some(program.into()));
    }
    if !auth.is_api_key_auth() {
        return Ok(None);
    }
    match policy {
        ApiKeyCyberAccessPrograms::UnsupportedProvider => Ok(None),
        ApiKeyCyberAccessPrograms::Disabled => Err(CodexErr::InvalidRequest(
            "Cyber access programs are disabled for this API-key session.".to_owned(),
        )),
        ApiKeyCyberAccessPrograms::Enabled => Ok(Some(program.into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn saved_daybreak_choice_uses_the_advertised_program_without_silent_downgrade() {
        let mut model = codex_models_manager::model_info::model_info_from_slug("test-model");
        for (programs, expected) in [
            (None, CyberAccessProgram::DaybreakBlue),
            (Some(vec![]), CyberAccessProgram::DaybreakBlue),
            (
                Some(vec![CyberAccessProgram::Standard]),
                CyberAccessProgram::DaybreakBlue,
            ),
            (
                Some(vec![
                    CyberAccessProgram::Standard,
                    CyberAccessProgram::DaybreakRed,
                ]),
                CyberAccessProgram::DaybreakRed,
            ),
            (
                Some(vec![
                    CyberAccessProgram::DaybreakBlue,
                    CyberAccessProgram::DaybreakRed,
                ]),
                CyberAccessProgram::DaybreakBlue,
            ),
        ] {
            model.available_access_programs = programs.map(|cyber| ModelAccessPrograms { cyber });
            assert_eq!(saved_thread_program(&model, true), expected);
            assert_eq!(
                saved_thread_program(&model, false),
                CyberAccessProgram::Standard
            );
        }
    }
}

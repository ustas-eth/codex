//! Forward explicit programs while leaving entitlement and model policy to the server.

use codex_api::AccessPrograms;
use codex_config::CyberAccessProgramPreference;
use codex_features::Feature;
use codex_login::CodexAuth;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
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
/// An explicit request wins; `auto` suppresses both configuration and inheritance.
pub(crate) fn for_turn(
    config: &crate::config::Config,
    model: &str,
    auth: Option<&CodexAuth>,
    explicit: Option<CyberAccessProgram>,
    inherited: Option<CyberAccessProgram>,
) -> Option<CyberAccessProgram> {
    let preference = auth.filter(|auth| auth.is_chatgpt_auth()).and_then(|_| {
        config
            .cyber_access_program_by_model
            .get(model)
            .copied()
            .or(config.cyber_access_program)
    });
    for_provider(
        &config.model_provider_id,
        explicit.or(match preference {
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

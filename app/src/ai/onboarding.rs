//! Onboarding-specific AI types and conversions.

use ai::LLMId;
use onboarding::slides::OnboardingModelInfo;
use onboarding::OnboardingAuthState;
use warp_core::ui::icons::Icon;
use warpui::{AppContext, SingletonEntity};

use super::execution_profiles::model_menu_items::is_auto;
use super::llms::{DisableReason, LLMInfo, LLMPreferences};
use crate::auth::AuthStateProvider;
use crate::workspaces::user_workspaces::UserWorkspaces;

const DEFAULT_EFFORT_LABEL: &str = "Default";

fn onboarding_base_title(llm: &LLMInfo) -> String {
    if is_auto(llm) {
        "auto".to_string()
    } else if llm.has_reasoning_level() {
        llm.base_model_name().to_string()
    } else {
        llm.menu_display_name()
    }
}

fn onboarding_effort_title(llm: &LLMInfo) -> String {
    if is_auto(llm) && llm.display_name.starts_with("auto (") {
        llm.display_name
            .trim_start_matches("auto (")
            .trim_end_matches(')')
            .to_string()
    } else {
        llm.reasoning_level()
            .unwrap_or_else(|| DEFAULT_EFFORT_LABEL.to_string())
    }
}

impl From<&LLMInfo> for OnboardingModelInfo {
    fn from(llm: &LLMInfo) -> Self {
        Self {
            id: llm.id.clone(),
            title: llm.display_name.clone(),
            base_title: onboarding_base_title(llm),
            effort_title: onboarding_effort_title(llm),
            icon: llm.provider.icon().unwrap_or(Icon::Oz),
            requires_upgrade: matches!(llm.disable_reason, Some(DisableReason::RequiresUpgrade)),
            is_default: false,
        }
    }
}

pub fn build_onboarding_models(
    prefs: &LLMPreferences,
    app: &AppContext,
) -> (Vec<OnboardingModelInfo>, LLMId) {
    let default_id = prefs.get_default_base_model().id.clone();
    let models: Vec<OnboardingModelInfo> = prefs
        .get_base_llm_choices_for_agent_mode(app)
        .map(|llm| {
            let mut info = OnboardingModelInfo::from(llm);
            info.is_default = info.id == default_id;
            info
        })
        .collect();
    (models, default_id)
}

pub fn current_onboarding_auth_state(ctx: &AppContext) -> OnboardingAuthState {
    let auth_state = AuthStateProvider::as_ref(ctx).get();
    if auth_state.is_anonymous_or_logged_out() {
        return OnboardingAuthState::LoggedOut;
    }
    let is_on_paid_plan = UserWorkspaces::as_ref(ctx)
        .current_workspace()
        .map(|w| w.billing_metadata.is_user_on_paid_plan())
        .unwrap_or(false);
    if is_on_paid_plan {
        OnboardingAuthState::PayingUser
    } else {
        OnboardingAuthState::FreeUser
    }
}

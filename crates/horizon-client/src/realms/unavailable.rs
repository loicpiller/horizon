//! What `RealmsMainScreen` does around `RealmsAvailability` (`init`'s
//! `availability.thenAcceptAsync`): the list shows LOADING and asks nothing of the service
//! until the check says yes; any other answer replaces it with the screen
//! `RealmsAvailability.Result.createErrorScreen` picks, whose Back (or OK) and Escape return
//! to the title screen, the main screen's `lastScreen`. The generic error screen
//! (`RealmsGenericErrorScreen`) is drawn here for every Realms error.
//!
//! Horizon differs in one place: without a usable session (signed out, or the token
//! expired) nothing is checked and the list keeps Horizon's "sign in with the launcher"
//! message, where vanilla's only session-less case, offline developer mode, opens the
//! "Invalid Session" screen. That screen tells the player to restart the game, which can't
//! help someone who isn't signed in. A session the service refuses (401) gets vanilla's
//! screen.
use super::availability::{self, Availability};
use super::{FetchState, Outcome, Screen, SubScreen, centered, session, wrap_lines};
use crate::hud::{HudAtlas, HudBatch};
use crate::text::Text;
use horizon_core::version;

/// `CommonLinks.GDPR`: `RealmsParentalConsentScreen`'s privacy button.
const GDPR: &str = "https://aka.ms/MinecraftGDPR";
/// The parental consent screen's privacy button.
pub(super) const PRIVACY_INFO: u8 = 250;
/// `Screen.buttons`' rows.
type Buttons = Vec<(f32, f32, f32, String, bool, u8, Option<(&'static str, f32)>)>;

/// The screen that replaced the main list (`createErrorScreen`).
#[derive(Clone, Debug)]
pub enum Unavailable {
    /// `RealmsClientOutdatedScreen`.
    Outdated,
    /// `RealmsParentalConsentScreen`, with its button row's top as last drawn (it sits
    /// under the wrapped message).
    ParentalConsent(std::cell::Cell<f32>),
    /// `RealmsGenericErrorScreen`: the invalid session's, or the unexpected error's.
    Error { title: String, detail: String },
}

impl Unavailable {
    /// `RealmsAvailability.Result.createErrorScreen`: `None` for a success.
    pub fn for_result(result: &Availability) -> Option<Self> {
        let t = crate::lang::get;
        Some(match result {
            Availability::Success => return None,
            Availability::IncompatibleClient => Self::Outdated,
            Availability::NeedsParentalConsent => Self::ParentalConsent(std::cell::Cell::new(0.0)),
            Availability::AuthenticationError => Self::Error {
                title: t("mco.error.invalid.session.title"),
                detail: t("mco.error.invalid.session.message"),
            },
            // `ErrorMessage.forServiceError`: the code in the title, the message below.
            Availability::UnexpectedError(e) => Self::Error {
                title: crate::lang::tr(
                    "mco.errorMessage.realmsService.realmsError",
                    &[&e.code() as &dyn std::fmt::Display],
                ),
                detail: e.message(),
            },
        })
    }

    /// Each screen's `init` buttons; Back and OK are the main screen's Back (id 2): they
    /// all close to the title.
    pub(super) fn buttons(&self, buttons: &mut Buttons, w: f32, h: f32) {
        let cx = (w / 2.0).floor();
        let t = crate::lang::get;
        match self {
            // The 200-wide Back centred in the 33-tall footer.
            Self::Outdated => {
                buttons.push((cx - 100.0, h - 27.0, 200.0, t("gui.back"), true, 2, None))
            }
            // Two 150-wide buttons 8 apart, the privacy link first.
            Self::ParentalConsent(row) => {
                let label = t("mco.account.privacy.info.button");
                buttons.push((
                    cx - 154.0,
                    row.get(),
                    150.0,
                    label,
                    true,
                    PRIVACY_INFO,
                    None,
                ));
                buttons.push((cx + 4.0, row.get(), 150.0, t("gui.back"), true, 2, None));
            }
            Self::Error { .. } => {
                buttons.push((cx - 100.0, h - 52.0, 200.0, t("gui.ok"), true, 2, None))
            }
        }
    }

    pub(super) fn draw(&self, batch: &mut HudBatch, atlas: &HudAtlas, w: f32, h: f32) {
        let font = &atlas.font;
        // `MultiLineTextWidget.setCentered`: each line centred on the widest one's middle,
        // the widget centred in the screen's width.
        let centred_lines = |batch: &mut HudBatch, lines: &[Text], y: f32| {
            let widest = lines.iter().map(|l| font.width_text(l)).fold(0.0, f32::max);
            let mid = ((w - widest) / 2.0).floor() + (widest / 2.0).floor();
            for (i, line) in lines.iter().enumerate() {
                let x = mid - (font.width_text(line) / 2.0).floor();
                batch.text_styled(atlas, line, x, y + 9.0 * i as f32, u32::MAX, true);
            }
        };
        let lines = |text: &Text, width: f32| -> Vec<Text> {
            font.split_lines(text, width)
                .into_iter()
                .map(|(l, _)| l)
                .collect()
        };
        match self {
            Self::Outdated => {
                // `addTitleHeader`: the red title centred in the 33-tall header.
                let title = crate::lang::get("mco.client.incompatible.title");
                centered(batch, atlas, &title, (w / 2.0).floor(), 12.0, super::RED);
                // The version name in red (-65536) inside the white message, split at its
                // line breaks only; the contents sit 30 under the header, or higher when
                // the footer would cover them.
                let key = if version::STABLE {
                    "mco.client.outdated.stable.version"
                } else {
                    "mco.client.unsupported.snapshot.version"
                };
                let name = Text::colored(version::VANILLA_VERSION, 0xFF_0000);
                let message = lines(&Text::translate(key, &[name]), f32::MAX);
                let y = 63.0_f32.min(h - 33.0 - 9.0 * message.len() as f32);
                centred_lines(batch, &message, y);
            }
            Self::ParentalConsent(row) => {
                // A vertical layout 15 apart, centred in the screen: the message wrapped
                // at width - 15, then the button row.
                let text = Text::translate("mco.account.privacy.information", &[]);
                let message = lines(&text, w - 15.0);
                let height = 9.0 * message.len() as f32;
                let top = ((h - (height + 15.0 + 20.0)) / 2.0).floor();
                centred_lines(batch, &message, top);
                row.set(top + height + 15.0);
            }
            Self::Error { title, detail } => draw_generic_error(batch, atlas, w, title, detail),
        }
    }

    /// Each screen's `getNarrationMessage`: the outdated screen's title (`Screen`'s own),
    /// the consent screen's message alone, the generic error's title and detail
    /// (`joinForNarration`).
    fn narration(&self) -> String {
        match self {
            Self::Outdated => crate::lang::get("mco.client.incompatible.title"),
            Self::ParentalConsent(_) => crate::lang::get("mco.account.privacy.information"),
            Self::Error { title, detail } => crate::widgets::join_for_narration(&[title, detail]),
        }
    }
}

/// `RealmsGenericErrorScreen.extractRenderState`.
pub(super) fn draw_generic_error(
    batch: &mut HudBatch,
    atlas: &HudAtlas,
    w: f32,
    title: &str,
    detail: &str,
) {
    let cx = (w / 2.0).floor();
    // `RealmsGenericErrorScreen`: the title at 80, the detail from 100 in the
    // -2142128 red, wrapped at 3/4 of the screen, 9 apart.
    centered(batch, atlas, title, cx, 80.0, u32::MAX);
    for (i, line) in wrap_lines(detail, atlas, w * 3.0 / 4.0).iter().enumerate() {
        centered(batch, atlas, line, cx, 100.0 + i as f32 * 9.0, 0xFF50_50DF);
    }
}

impl Screen {
    /// `LongRunningTask.error`: a task's failure replaces the screen with
    /// `RealmsGenericErrorScreen(new Component(message), nextScreen)` — the generic title
    /// and the message as its detail; OK returns to the list.
    pub(super) fn show_generic_error(&mut self, detail: String) {
        self.subscreen = SubScreen::Error {
            title: crate::lang::get("mco.errorMessage.generic"),
            detail,
            next: Box::new(SubScreen::Main),
        };
    }

    /// The screen the narrator reads: a refusal is a screen of its own (narrated as it opens,
    /// with its `getNarrationMessage`), anything else the main screen's title.
    pub fn screen_info(&self) -> crate::widgets::ScreenInfo {
        use crate::widgets::ScreenInfo;
        match &self.subscreen {
            SubScreen::Unavailable(screen) => {
                ScreenInfo::new("realms/unavailable", screen.narration())
            }
            _ => ScreenInfo::new("realms", crate::lang::get("menu.online")),
        }
    }

    /// `RealmsMainScreen.init`: true while the data requests wait for the availability
    /// check, which runs (the list shows LOADING) or failed (its screen replaced the list).
    /// The check this screen already waits on is kept; otherwise `RealmsAvailability.get()`.
    pub(super) fn hold_for_availability(&mut self) -> bool {
        let pending = match &self.state {
            FetchState::Checking(pending) => pending.clone(),
            _ => match session() {
                Some(session) => availability::get(Some(&session)),
                // Signed out: Horizon's message, nothing is asked (the module docs).
                None => return false,
            },
        };
        let answer = pending.now();
        if answer == Some(Availability::Success) {
            return false;
        }
        if let Some(screen) = answer.as_ref().and_then(Unavailable::for_result) {
            self.subscreen = SubScreen::Unavailable(screen);
        }
        self.state = FetchState::Checking(pending);
        true
    }

    /// The main list's frame: the check's answer, when it comes, starts the data requests
    /// (`initDataFetcher`) or opens its screen. True when the data fetcher may tick.
    pub(super) fn poll_availability(&mut self) -> bool {
        if !matches!(self.state, FetchState::Checking(_)) {
            return true;
        }
        if self.hold_for_availability() {
            return false;
        }
        self.refresh();
        true
    }

    /// `RealmsParentalConsentScreen`'s privacy button: `ConfirmLinkScreen.confirmLink`,
    /// which returns to the consent screen.
    pub(super) fn privacy_info(&mut self) -> Outcome {
        self.open_link(GDPR.to_owned());
        Outcome::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::realms::Popup;
    use crate::realms::availability::Pending;
    use crate::realms::client::RealmsError;
    use std::time::{Duration, Instant};

    fn checking(answer: Option<Availability>) -> Screen {
        let mut screen = Screen::new();
        let pending = Pending::default();
        if let Some(answer) = answer {
            pending.resolve(answer);
        }
        screen.state = FetchState::Checking(pending);
        screen
    }

    fn no_data_job(screen: &Screen) -> bool {
        screen.news_job.is_none()
            && screen.notifications_job.is_none()
            && screen.pending_invites_job.is_none()
            && screen.trial_job.is_none()
            && screen.live_job.is_none()
    }

    #[test]
    fn nothing_is_fetched_until_the_check_says_yes() {
        let mut screen = checking(None);
        // Every `RealmsDataFetcher` period is long over: only the wait holds them back.
        let long_ago = Instant::now() - Duration::from_secs(3600);
        screen.last_list_refresh = long_ago;
        screen.last_pending_refresh = long_ago;
        screen.last_news_refresh = long_ago;
        screen.refresh();
        for _ in 0..3 {
            screen.poll();
        }
        assert!(matches!(screen.state, FetchState::Checking(_)));
        assert!(no_data_job(&screen));
        // LOADING meanwhile: the loading text, Add Realm and Join Realm off.
        assert_eq!(screen.layout_state(), super::super::LayoutState::Loading);
        let buttons = screen.buttons(800.0, 600.0);
        assert_eq!(buttons.iter().find(|b| b.5 == 13).map(|b| b.4), Some(false));
        assert!(matches!(screen.subscreen, SubScreen::Main));

        let FetchState::Checking(pending) = &screen.state else {
            unreachable!()
        };
        pending.resolve(Availability::Success);
        screen.poll();
        // `initDataFetcher` ran: the list is no longer waiting on the check (without a
        // session in tests, it settles on the signed-out state).
        assert!(matches!(screen.state, FetchState::Unauthenticated));
        assert!(matches!(screen.subscreen, SubScreen::Main));
    }

    #[test]
    fn each_refusal_opens_its_screen_and_every_way_out_is_the_title() {
        let refused = RealmsError::Connectivity("refused".into());
        let cases = [
            (Availability::IncompatibleClient, "outdated"),
            (Availability::NeedsParentalConsent, "consent"),
            (
                Availability::AuthenticationError,
                "mco.error.invalid.session.title",
            ),
            (
                Availability::UnexpectedError(refused.clone()),
                "mco.errorMessage.realmsService.realmsError",
            ),
        ];
        for (answer, expected) in cases {
            let mut screen = checking(Some(answer.clone()));
            screen.poll();
            let SubScreen::Unavailable(shown) = &screen.subscreen else {
                panic!("{answer:?} kept the list");
            };
            match (shown, expected) {
                (Unavailable::Outdated, "outdated") => {}
                (Unavailable::ParentalConsent(_), "consent") => {}
                (Unavailable::Error { title, detail }, _) if title.starts_with(expected) => {
                    if answer == Availability::AuthenticationError {
                        assert_eq!(detail, "mco.error.invalid.session.message");
                    } else {
                        assert_eq!(detail, &refused.message());
                    }
                }
                _ => panic!("{answer:?} opened {shown:?}"),
            }
            assert!(no_data_job(&screen));
            let back = if matches!(shown, Unavailable::Error { .. }) {
                "gui.ok"
            } else {
                "gui.back"
            };
            let buttons = screen.buttons(800.0, 600.0);
            assert!(
                buttons.iter().any(|b| b.5 == 2 && b.3 == back && b.4),
                "{answer:?}"
            );
            assert!(matches!(
                screen.clone().click(Some(2), 0.0, 0.0, (800.0, 600.0)),
                Outcome::Back
            ));
            assert!(screen.escape(), "{answer:?}: Escape closes to the title");
        }
    }

    #[test]
    fn an_answer_while_another_screen_is_open_waits_for_the_list() {
        for answer in [Availability::IncompatibleClient, Availability::Success] {
            let mut screen = checking(None);
            // The invitations button stays usable during the check.
            screen.click(Some(15), 0.0, 0.0, (800.0, 600.0));
            assert!(matches!(screen.subscreen, SubScreen::PendingInvites { .. }));
            let FetchState::Checking(pending) = &screen.state else {
                unreachable!()
            };
            pending.resolve(answer.clone());
            screen.poll();
            // `thenAcceptAsync(.., screenExecutor)`: nothing happens while the list is away.
            assert!(
                matches!(screen.subscreen, SubScreen::PendingInvites { .. }),
                "{answer:?}"
            );
            assert!(matches!(screen.state, FetchState::Checking(_)));
            assert!(!screen.escape(), "{answer:?}: Escape returns to the list");
            screen.poll();
            if answer == Availability::Success {
                assert!(matches!(screen.subscreen, SubScreen::Main));
                assert!(!matches!(screen.state, FetchState::Checking(_)));
            } else {
                assert!(matches!(
                    screen.subscreen,
                    SubScreen::Unavailable(Unavailable::Outdated)
                ));
            }
        }
    }

    /// Strings of this test's own for the keys the Realms screens narrate.
    struct Strings;

    impl crate::lang::ResourceSource for Strings {
        fn read(&self, path: &str) -> Option<Vec<u8>> {
            (path == "assets/minecraft/lang/en_us.json").then(|| {
                br#"{
                    "menu.online": "Realms Online",
                    "mco.client.incompatible.title": "Too old",
                    "mco.account.privacy.information": "Ask a parent",
                    "mco.error.invalid.session.title": "Bad session",
                    "mco.error.invalid.session.message": "Restart",
                    "mco.errorMessage.realmsService.realmsError": "Error %s",
                    "mco.errorMessage.realmsService.connectivity": "No connection: %s"
                }"#
                .to_vec()
            })
        }
        fn namespaces(&self) -> Vec<String> {
            vec!["minecraft".into()]
        }
    }

    #[test]
    fn each_refusal_screen_narrates_vanillas_message() {
        let cases = [
            (None, "realms", "Realms Online"),
            // `RealmsClientOutdatedScreen`: `Screen.getNarrationMessage`, its title.
            (
                Some(Availability::IncompatibleClient),
                "realms/unavailable",
                "Too old",
            ),
            // `RealmsParentalConsentScreen`: its message alone.
            (
                Some(Availability::NeedsParentalConsent),
                "realms/unavailable",
                "Ask a parent",
            ),
            // `RealmsGenericErrorScreen`: `joinForNarration(title, detail)`.
            (
                Some(Availability::AuthenticationError),
                "realms/unavailable",
                "Bad session. Restart",
            ),
            (
                Some(Availability::UnexpectedError(RealmsError::Connectivity(
                    "refused".into(),
                ))),
                "realms/unavailable",
                "Error 500. No connection: refused",
            ),
        ];
        for (answer, id, expected) in cases {
            let english = crate::lang::Language::load(&[&Strings], "en_us");
            let (info, missed) = crate::lang::scoped(english, || {
                let mut screen = checking(answer.clone());
                screen.poll();
                screen.screen_info()
            });
            assert_eq!((info.id.as_str(), info.title.as_str()), (id, expected));
            assert!(missed.is_empty(), "{answer:?}: {missed:?}");
        }
    }

    #[test]
    fn a_known_refusal_shows_its_screen_at_once() {
        // Reopening after a kept answer: the screen opens without waiting a frame.
        let mut screen = checking(Some(Availability::IncompatibleClient));
        screen.refresh();
        assert!(matches!(
            screen.subscreen,
            SubScreen::Unavailable(Unavailable::Outdated)
        ));
    }

    #[test]
    fn the_privacy_button_asks_before_the_link_and_returns_to_the_consent_screen() {
        let mut screen = checking(Some(Availability::NeedsParentalConsent));
        screen.poll();
        let buttons = screen.buttons(800.0, 600.0);
        assert!(buttons.iter().any(|b| b.5 == PRIVACY_INFO && b.4));
        screen.click(Some(PRIVACY_INFO), 0.0, 0.0, (800.0, 600.0));
        let SubScreen::Popup(Popup::OpenLink { url, parent, .. }) = &screen.subscreen else {
            panic!("no link question: {:?}", screen.subscreen);
        };
        assert_eq!(url, "https://aka.ms/MinecraftGDPR");
        assert!(matches!(
            **parent,
            SubScreen::Unavailable(Unavailable::ParentalConsent(_))
        ));
        assert!(!screen.escape());
        assert!(matches!(
            screen.subscreen,
            SubScreen::Unavailable(Unavailable::ParentalConsent(_))
        ));
    }
}

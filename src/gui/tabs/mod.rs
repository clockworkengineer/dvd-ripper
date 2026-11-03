//! Tab navigation router and active view definitions.
//! Follows Jakob Nielsen's Heuristic #7 (Flexibility & Efficiency) and #8 (Aesthetic & Minimalist Design).

pub mod benchmark_tab;
pub mod history_tab;
pub mod rip_tab;
pub mod settings_tab;
pub mod tv_tab;

use eframe::egui;

/// Active top-level workflow tab in the GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActiveTab {
    #[default]
    Rip,
    TvSeries,
    Benchmark,
    History,
    Settings,
}

impl ActiveTab {
    /// Human-friendly display label with icon.
    pub fn label(&self) -> &'static str {
        match self {
            Self::Rip => "🎬 Rip Disc",
            Self::TvSeries => "📺 TV Box Sets",
            Self::Benchmark => "⚡ Benchmark",
            Self::History => "📜 History",
            Self::Settings => "⚙️ Settings",
        }
    }

    /// Short tooltip describing the purpose of the tab.
    pub fn tooltip(&self) -> &'static str {
        match self {
            Self::Rip => "Single movie and 1:1 ISO dump extraction",
            Self::TvSeries => "Multi-episode TV series season box set batch ripper",
            Self::Benchmark => "Optical drive read throughput and speed diagnostic",
            Self::History => "Persistent ripping activity log and recorded files",
            Self::Settings => "Global configuration, hardware acceleration, and defaults",
        }
    }
}

/// Renders the horizontal tab navigation bar.
pub fn render_tab_bar(ui: &mut egui::Ui, current_tab: &mut ActiveTab, is_busy: bool) {
    ui.horizontal(|ui| {
        let tabs = [
            ActiveTab::Rip,
            ActiveTab::TvSeries,
            ActiveTab::Benchmark,
            ActiveTab::History,
            ActiveTab::Settings,
        ];

        for tab in tabs {
            let is_selected = *current_tab == tab;
            let rich_text = if is_selected {
                egui::RichText::new(tab.label())
                    .strong()
                    .color(super::theme::ACCENT_COLOR)
            } else {
                egui::RichText::new(tab.label())
            };

            let resp = ui
                .add_enabled(
                    !is_busy || tab == ActiveTab::History,
                    egui::Button::new(rich_text),
                )
                .on_hover_text(tab.tooltip());
            if resp.clicked() {
                *current_tab = tab;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_tab_labels_and_tooltips() {
        assert_eq!(ActiveTab::default(), ActiveTab::Rip);
        let all_tabs = [
            ActiveTab::Rip,
            ActiveTab::TvSeries,
            ActiveTab::Benchmark,
            ActiveTab::History,
            ActiveTab::Settings,
        ];
        for tab in all_tabs {
            assert!(!tab.label().is_empty());
            assert!(!tab.tooltip().is_empty());
        }
    }

    #[test]
    fn test_render_tab_bar_headless() {
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut current = ActiveTab::Rip;
                render_tab_bar(ui, &mut current, false);
                render_tab_bar(ui, &mut current, true);
            });
        });
    }
}

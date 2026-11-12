use super::*;
use crate::workflow::{AgentRole, AgentStatus, CheckStatus};
use egui::RichText;
use theme::*;

impl Workspace {
    pub(super) fn agent_panel(&mut self, ctx: &egui::Context) {
        if ctx.screen_rect().width() < 1180.0 || (!self.show_agents && !self.tasks.is_busy()) {
            return;
        }
        egui::SidePanel::right("agent-team").exact_width(240.0).resizable(false)
            .frame(egui::Frame::new().fill(PANEL).inner_margin(18)).show(ctx, |ui| {
                ui.add_space(10.0); ui.label(RichText::new("YOUR AGENT TEAM").small().strong().color(MUTED));
                ui.add_space(10.0);
                for (role,label,description) in [(AgentRole::Planner,"Planner","Requirements & approach"),(AgentRole::Developer,"Developer","Application source"),(AgentRole::Reviewer,"Reviewer","Independent AI advisory"),(AgentRole::Tester,"QA analyst","Deterministic source checks"),(AgentRole::Deployer,"Release manager","Packaging & publishing")] {
                    let agent=self.report.as_ref().and_then(|r|r.agents.iter().find(|a|a.role==role));
                    let (state,color)=match agent.map(|a|a.status) { Some(AgentStatus::Completed)=>("Completed",GREEN),Some(AgentStatus::NeedsAttention)=>("Needs attention",AMBER),Some(AgentStatus::AwaitingConfirmation)=>("Awaiting approval",AMBER),_=>("Not run",MUTED) };
                    egui::Frame::new().fill(CANVAS).corner_radius(egui::CornerRadius::same(9)).inner_margin(12).show(ui, |ui| {
                        ui.set_min_width(178.0); ui.strong(label); ui.label(RichText::new(description).small().color(MUTED)); ui.label(RichText::new(state).small().color(color));
                    }); ui.add_space(4.0);
                }
                ui.add_space(14.0);
                if self.tasks.is_busy() { ui.spinner(); ui.label(RichText::new("An operation is running. Reports appear when it completes.").small().color(ACCENT)); }
                else { ui.label(RichText::new("Specialist roles. One orchestrated workflow.\n\nAI advice is separate from runtime verification.").small().color(MUTED)); }
            });
    }
    pub(super) fn testing_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        title(
            ui,
            "QUALITY WORKSPACE",
            "Confidence comes from evidence",
            "Review deterministic source checks and the agent team's findings.",
        );
        let Some(_) = &self.selected else {
            card().show(ui, |ui| {
                ui.label("Generate or select a project to start reviewing its source.");
                if ui.button("Go to development").clicked() {
                    self.tab = 0;
                }
            });
            return;
        };
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(
                    !self.tasks.is_busy() && !self.dirty,
                    primary("Run static checks"),
                )
                .clicked()
            {
                let snapshot = self.selected.as_ref().expect("selected project").clone();
                self.launch(ctx.clone(), async move {
                    Ok(Event::Reviewed(crate::workflow::test_project(&snapshot)))
                });
            }
            ui.label(
                RichText::new("Runtime execution: not verified")
                    .color(AMBER)
                    .small(),
            );
        });
        if self.dirty {
            ui.label(
                RichText::new("Save your source changes before checking this revision.")
                    .color(AMBER),
            );
        }
        ui.add_space(14.0);
        egui::ScrollArea::vertical().show(ui, |ui| {
            if let Some(report)=&self.report {
                for check in &report.checks {
                    let (label,color)=match check.status {CheckStatus::Passed=>("PASS",GREEN),CheckStatus::Failed=>("FAIL",egui::Color32::LIGHT_RED),CheckStatus::Warning=>("REVIEW",AMBER),CheckStatus::NotRun=>("NOT RUN",MUTED)};
                    card().show(ui, |ui| {ui.set_min_width(ui.available_width()); ui.horizontal_wrapped(|ui| {ui.label(RichText::new(label).small().strong().color(color));ui.strong(&check.name);});ui.label(RichText::new(&check.detail).color(MUTED));}); ui.add_space(8.0);
                }
                ui.add_space(12.0);ui.heading("Agent findings");
                for agent in &report.agents { egui::CollapsingHeader::new(format!("{:?} / {:?}",agent.role,agent.status)).default_open(agent.role==AgentRole::Reviewer).show(ui, |ui|{ui.label(&agent.summary);}); }
            } else {card().show(ui, |ui| {ui.label("No report for this source revision yet.");ui.label(RichText::new("Checks inspect source structure and selected risk patterns. They do not execute the application.").color(MUTED));});}
        });
    }
    pub(super) fn deployment_view(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        title(
            ui,
            "RELEASE WORKSPACE",
            "Ship with intention",
            "Package your source locally, or publish through an explicitly enabled Vercel connection.",
        );
        let Some(_) = &self.selected else {
            card().show(ui, |ui| {
                ui.label("Choose a project before preparing a release.");
            });
            return;
        };
        let mut export = false;
        let mut deploy = false;
        egui::ScrollArea::vertical().show(ui, |ui| {
            card().show(ui, |ui| {
                ui.set_min_width(ui.available_width());ui.label(RichText::new("01 / SOURCE BUNDLE").small().color(ACCENT));ui.heading("Keep a local release");ui.label("Export source with a manifest and static review report. No code is executed.");
                ui.checkbox(&mut self.export_confirmed,"Create a new source bundle on this computer");
                export=ui.add_enabled(!self.tasks.is_busy() && !self.dirty && self.export_confirmed,primary("Export source bundle")).clicked();
                if let Some(path)=&self.export_path {ui.label(RichText::new(path.display().to_string()).color(GREEN));}
            });
            ui.add_space(16.0);
            card().show(ui, |ui| {
                ui.set_min_width(ui.available_width());ui.label(RichText::new("02 / CLOUD DEPLOYMENT").small().color(ACCENT));ui.heading("Publish to Vercel");
                let configured=crate::release::configured();
                ui.label(RichText::new(if configured {"Vercel credentials configured"} else {"Vercel is not configured"}).color(if configured {GREEN}else{AMBER}));
                if !configured {ui.label("Set VERCEL_TOKEN and TESSERUN_ENABLE_VERCEL_DEPLOYMENTS=true before starting the app. Optional: VERCEL_PROJECT_ID / VERCEL_TEAM_ID.");}
                ui.label(RichText::new("Application behavior and security have not been runtime-verified. Publishing makes these files publicly accessible.").color(AMBER));
                ui.checkbox(&mut self.deploy_confirmed,"I approve publishing this unverified source to production");
                deploy=ui.add_enabled(configured && !self.tasks.is_busy() && !self.dirty && self.deploy_confirmed,primary("Publish to Vercel")).clicked();
                if let Some(deployment)=&self.deployment {ui.label(format!("Deployment {}",deployment.status)); ui.hyperlink_to(&deployment.url,&deployment.url);}
            });
            if self.dirty {ui.label(RichText::new("Save changes before packaging or publishing.").color(AMBER));}
        });
        if export || deploy {
            // Each release action needs a fresh, explicit confirmation.
            self.export_confirmed = false;
            self.deploy_confirmed = false;
        }
        if export {
            let snapshot = self.selected.as_ref().expect("selected project").clone();
            self.launch(ctx.clone(), async move {
                Ok(Event::Exported(
                    crate::workflow::export_project(
                        &snapshot,
                        std::path::PathBuf::from(".tesserun-rust/exports"),
                        true,
                    )
                    .await?,
                ))
            });
        } else if deploy {
            let snapshot = self.selected.as_ref().expect("selected project").clone();
            self.launch(ctx.clone(), async move {
                Ok(Event::Deployed(
                    crate::release::deploy(snapshot, true).await?,
                ))
            });
        }
    }
}

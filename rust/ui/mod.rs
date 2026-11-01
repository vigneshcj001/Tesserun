use crate::{
    domain::{Message, Project},
    providers::Connection,
    service::Service,
};
use eframe::egui;
mod tasks;
mod theme;
mod views;
mod workflow_views;
use tasks::TaskRunner;

enum Event {
    Projects(Vec<Project>),
    Developed(crate::workflow::DevelopmentRun),
    Reviewed(crate::workflow::WorkflowReport),
    Exported(std::path::PathBuf),
    Deployed(crate::release::Deployment),
    Reply(String),
    Saved(Project),
    Failed(String),
}
pub struct Workspace {
    service: Service,
    tasks: TaskRunner<Event>,
    connection: Connection,
    projects: Vec<Project>,
    selected: Option<Project>,
    messages: Vec<Message>,
    prompt: String,
    draft: String,
    tab: usize,
    file: usize,
    status: String,
    report: Option<crate::workflow::WorkflowReport>,
    deployment: Option<crate::release::Deployment>,
    export_path: Option<std::path::PathBuf>,
    dirty: bool,
    export_confirmed: bool,
    deploy_confirmed: bool,
    theme_ready: bool,
    show_agents: bool,
}
impl Workspace {
    pub fn new(cc: &eframe::CreationContext<'_>) -> anyhow::Result<Self> {
        theme::configure_fonts(&cc.egui_ctx);
        theme::apply(&cc.egui_ctx);
        let tasks = TaskRunner::new()?;
        let root = std::env::var_os("TESSERUN_RUST_DATA_ROOT")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| ".tesserun-rust/projects".into());
        let service = Service::new(root)?;
        let mut app = Self {
            service,
            tasks,
            connection: Connection::default(),
            projects: Vec::new(),
            selected: None,
            messages: Vec::new(),
            prompt: String::new(),
            draft: String::new(),
            tab: 0,
            file: 0,
            status: "Ready. Configure a provider and model in Settings.".into(),
            report: None,
            deployment: None,
            export_path: None,
            dirty: false,
            export_confirmed: false,
            deploy_confirmed: false,
            theme_ready: false,
            show_agents: false,
        };
        app.refresh(cc.egui_ctx.clone());
        Ok(app)
    }
    fn launch<F>(&mut self, ctx: egui::Context, work: F)
    where
        F: std::future::Future<Output = anyhow::Result<Event>> + Send + 'static,
    {
        self.tasks.spawn(ctx, work);
        self.status = "Working…".into();
    }
    fn refresh(&mut self, ctx: egui::Context) {
        let store = self.service.store.clone();
        self.launch(ctx, async move { Ok(Event::Projects(store.list().await?)) });
    }
    fn record_release(&mut self, summary: String) {
        if self.report.is_none()
            && let Some(project) = &self.selected
        {
            self.report = Some(crate::workflow::test_project(project));
        }
        if let Some(report) = &mut self.report
            && let Some(agent) = report
                .agents
                .iter_mut()
                .find(|agent| agent.role == crate::workflow::AgentRole::Deployer)
        {
            agent.status = crate::workflow::AgentStatus::Completed;
            agent.summary = summary;
        }
    }
}
impl eframe::App for Workspace {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        // eframe can apply the OS theme after constructing the app.
        if !self.theme_ready || ctx.style().visuals.dark_mode {
            theme::apply(ctx);
            self.theme_ready = true;
        }
        while let Some(result) = self.tasks.poll() {
            let event = result.unwrap_or_else(Event::Failed);
            match event {
                Event::Projects(projects) => {
                    self.projects = projects;
                    self.status = "Projects loaded".into();
                }
                Event::Developed(run) => {
                    self.show_agents = true;
                    let project = run.project;
                    self.report = Some(run.report);
                    self.deployment = None;
                    self.export_path = None;
                    self.dirty = false;
                    self.projects.insert(0, project.clone());
                    self.selected = Some(project);
                    self.file = 0;
                    self.tab = 0;
                    self.status = "Source generated. Execution has not been verified.".into();
                }
                Event::Reviewed(mut report) => {
                    if let Some(previous) = &self.report {
                        for agent in &previous.agents {
                            if matches!(
                                agent.role,
                                crate::workflow::AgentRole::Planner
                                    | crate::workflow::AgentRole::Developer
                                    | crate::workflow::AgentRole::Reviewer
                            ) {
                                report.agents.retain(|current| current.role != agent.role);
                                report.agents.push(agent.clone());
                            }
                        }
                    }
                    self.report = Some(report);
                    self.status =
                        "Static source checks completed. Runtime tests were not executed.".into();
                }
                Event::Exported(path) => {
                    self.status = format!("Source bundle exported to {}", path.display());
                    self.record_release("Source bundle and manifest exported locally. No cloud publication or runtime verification was performed.".into());
                    self.export_path = Some(path);
                }
                Event::Deployed(deployment) => {
                    self.status = format!("Vercel deployment {}", deployment.status);
                    self.record_release(format!("Vercel accepted the production request. Deployment state: {}. Runtime verification was not performed.", deployment.status));
                    self.deployment = Some(deployment);
                }
                Event::Reply(content) => {
                    self.messages.push(Message {
                        role: "assistant".into(),
                        content,
                    });
                    self.status = "Reply received".into();
                }
                Event::Saved(project) => {
                    self.dirty = false;
                    if let Some(existing) =
                        self.projects.iter_mut().find(|item| item.id == project.id)
                    {
                        *existing = project;
                    }
                    self.status = "Revision saved".into();
                }
                Event::Failed(error) => self.status = error,
            }
        }
        self.navigation(ctx);
        self.top_bar(ctx);
        self.status_bar(ctx);
        self.agent_panel(ctx);
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(theme::CANVAS).inner_margin(24))
            .show(ctx, |ui| {
                let width = ui.available_width().min(980.0);
                let inset = ((ui.available_width() - width) / 2.0).max(0.0);
                ui.horizontal_top(|ui| {
                    ui.add_space(inset);
                    ui.vertical(|ui| {
                        ui.set_width(width);
                        match self.tab {
                            2 => self.settings_view(ui),
                            1 => self.chat_view(ui, ctx),
                            3 => self.testing_view(ui, ctx),
                            4 => self.deployment_view(ui, ctx),
                            _ => self.build_view(ui, ctx),
                        }
                    });
                });
            });
    }
}

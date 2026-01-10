//! Table Widget Example
//!
//! Demonstrates the Table widget with virtualized rows, columns, and frozen columns.

use std::fs::File;

use rafter::page;
use rafter::prelude::*;
use rafter::widgets::{Column, SelectionMode, Table, TableRow, TableState, Text};
use simplelog::{Config, LevelFilter, WriteLogger};
use tuidom::Element;

/// A user record for the table.
#[derive(Clone, Debug)]
struct User {
    id: u32,
    name: String,
    email: String,
    department: String,
    role: String,
    status: String,
    location: String,
    phone: String,
    hire_date: String,
    salary: String,
    manager: String,
    team: String,
    project: String,
}

impl TableRow for User {
    type Key = u32;

    fn key(&self) -> u32 {
        self.id
    }

    fn cell(&self, column_id: &str) -> Element {
        let text = match column_id {
            "id" => self.id.to_string(),
            "name" => self.name.clone(),
            "email" => self.email.clone(),
            "department" => self.department.clone(),
            "role" => self.role.clone(),
            "status" => self.status.clone(),
            "location" => self.location.clone(),
            "phone" => self.phone.clone(),
            "hire_date" => self.hire_date.clone(),
            "salary" => self.salary.clone(),
            "manager" => self.manager.clone(),
            "team" => self.team.clone(),
            "project" => self.project.clone(),
            _ => String::new(),
        };
        Element::text(&text)
    }
}

/// Create sample users for the table.
fn create_sample_users() -> Vec<User> {
    let departments = ["Engineering", "Sales", "Marketing", "HR", "Finance"];
    let roles = ["Manager", "Senior", "Junior", "Lead", "Intern"];
    let statuses = ["Active", "Away", "Busy", "Offline"];
    let first_names = ["Alice", "Bob", "Charlie", "Diana", "Eve", "Frank", "Grace", "Henry"];
    let last_names = ["Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller"];
    let locations = ["New York", "San Francisco", "London", "Tokyo", "Berlin", "Sydney"];
    let teams = ["Alpha", "Beta", "Gamma", "Delta", "Omega"];
    let managers = ["John Doe", "Jane Smith", "Bob Wilson", "Mary Johnson"];
    let projects = ["Phoenix", "Atlas", "Mercury", "Neptune", "Orion", "Titan"];

    let mut users = Vec::with_capacity(100);

    for i in 1..=100 {
        let first = first_names[i % first_names.len()];
        let last = last_names[i % last_names.len()];
        let name = format!("{} {}", first, last);
        let email = format!("{}.{}@example.com", first.to_lowercase(), last.to_lowercase());

        users.push(User {
            id: i as u32,
            name,
            email,
            department: departments[i % departments.len()].to_string(),
            role: roles[i % roles.len()].to_string(),
            status: statuses[i % statuses.len()].to_string(),
            location: locations[i % locations.len()].to_string(),
            phone: format!("+1-555-{:04}", 1000 + i),
            hire_date: format!("2020-{:02}-{:02}", (i % 12) + 1, (i % 28) + 1),
            salary: format!("${},000", 50 + (i % 100)),
            manager: managers[i % managers.len()].to_string(),
            team: teams[i % teams.len()].to_string(),
            project: projects[i % projects.len()].to_string(),
        });
    }

    users
}

/// Create column definitions.
fn create_columns() -> Vec<Column> {
    vec![
        Column::new("id", "ID").fixed(8),
        Column::new("name", "Name").fixed(25),
        Column::new("email", "Email").fixed(40),
        Column::new("department", "Department").fixed(20),
        Column::new("role", "Role").fixed(15),
        Column::new("status", "Status").fixed(15),
        Column::new("location", "Location").fixed(20),
        Column::new("phone", "Phone").fixed(20),
        Column::new("hire_date", "Hire Date").fixed(15),
        Column::new("salary", "Salary").fixed(15),
        Column::new("manager", "Manager").fixed(20),
        Column::new("team", "Team").fixed(15),
        Column::new("project", "Project").fixed(20),
    ]
}

#[app]
struct TableExample {
    users: TableState<User>,
    message: String,
}

#[app_impl]
impl TableExample {
    async fn on_start(&self) {
        let users = create_sample_users();
        let columns = create_columns();

        self.users.set(
            TableState::new(users, columns)
                .with_selection(SelectionMode::Single)
                .with_frozen(&["id", "name"]), // Freeze ID and Name columns
        );
        self.message
            .set("Navigate with arrows, Enter to select".into());
    }

    #[keybinds]
    fn keys() {
        bind("q", quit);
    }

    #[handler]
    async fn quit(&self, gx: &GlobalContext) {
        gx.shutdown();
    }

    #[handler]
    async fn row_selected(&self) {
        let state = self.users.get();
        if let Some(key) = &state.last_activated {
            self.message.set(format!("Selected user ID: {}", key));
        }
    }

    #[handler]
    async fn row_activated(&self, gx: &GlobalContext) {
        let state = self.users.get();
        if let Some(key) = &state.last_activated {
            if let Some(user) = state.rows.iter().find(|u| &u.id == key) {
                gx.toast(Toast::info(format!(
                    "Activated: {} ({})",
                    user.name, user.email
                )));
            }
        }
    }

    fn element(&self) -> Element {
        let message = self.message.get();

        page! {
            column (padding: 2, gap: 1, height: fill, width: fill) style (bg: background) {
                // Header
                text (content: "Table Widget Demo") style (bold, fg: accent)
                text (content: "ID and Name columns are frozen (always visible)") style (fg: muted)

                // Status
                row (gap: 1) {
                    text (content: "Status:") style (fg: muted)
                    text (content: {message}) style (fg: accent)
                }

                // Table view
                box_ (id: "table-container", height: fill, width: fill) style (bg: surface) {
                    table (state: self.users, id: "user-table")
                        on_select: row_selected()
                        on_activate: row_activated()
                }

                // Footer
                row (gap: 2) {
                    text (content: "Press 'q' to quit") style (fg: muted)
                    text (content: "| Up/Down: navigate | Enter: select") style (fg: muted)
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    // Set up file logging
    let log_file = File::create("table.log").expect("Failed to create log file");
    WriteLogger::init(LevelFilter::Debug, Config::default(), log_file)
        .expect("Failed to initialize logger");

    if let Err(e) = Runtime::new()
        .expect("Failed to create runtime")
        .run(TableExample::default())
        .await
    {
        eprintln!("Error: {}", e);
    }
}

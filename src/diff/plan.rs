use colored::Colorize;

use super::actions::{Action, ResourcePlan};

pub fn display_plan(actions: &[Action]) {
    let creates = actions.iter().filter(|a| matches!(a, Action::Create(_))).count();
    let updates = actions.iter().filter(|a| matches!(a, Action::Update(_))).count();
    let deletes = actions.iter().filter(|a| matches!(a, Action::Delete(_))).count();

    if creates == 0 && updates == 0 && deletes == 0 {
        println!("\n{}", "No changes. Infrastructure is up-to-date.".green());
        return;
    }

    println!("\n{}", "Telegram-tf will perform the following actions:".bold());
    println!();

    for action in actions {
        match action {
            Action::Create(plan) => display_create(plan),
            Action::Update(plan) => display_update(plan),
            Action::Delete(plan) => display_delete(plan),
            Action::NoOp => {}
        }
    }

    println!();
    println!(
        "Plan: {} to add, {} to change, {} to destroy.",
        creates.to_string().green(),
        updates.to_string().yellow(),
        deletes.to_string().red()
    );
}

fn display_create(plan: &ResourcePlan) {
    println!(
        "  {} {}",
        "+".green(),
        plan.resource_key.green().bold()
    );
    for change in &plan.changes {
        println!(
            "      {} {}: {}",
            "+".green(),
            change.field,
            change.new.as_deref().unwrap_or("(none)")
        );
    }
    println!();
}

fn display_update(plan: &ResourcePlan) {
    println!(
        "  {} {}",
        "~".yellow(),
        plan.resource_key.yellow().bold()
    );
    for change in &plan.changes {
        println!(
            "      {} {}: {} -> {}",
            "~".yellow(),
            change.field,
            change.old.as_deref().unwrap_or("(none)"),
            change.new.as_deref().unwrap_or("(none)")
        );
    }
    println!();
}

fn display_delete(plan: &ResourcePlan) {
    println!(
        "  {} {}",
        "-".red(),
        plan.resource_key.red().bold()
    );
    for change in &plan.changes {
        println!(
            "      {} {}: {}",
            "-".red(),
            change.field,
            change.old.as_deref().unwrap_or("(none)")
        );
    }
    println!();
}

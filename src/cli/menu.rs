use anyhow::Result;
use std::io::{self, Write};
use std::path::Path;

use crate::config::Configuration;
use crate::models::chart::ChartTaskContext;
use crate::models::date::get_best_chart_date;
use crate::pipeline::runner::run_processing;

fn prompt_line(prompt_text: &str) -> String {
    print!("{}", prompt_text);
    let _ = io::stdout().flush();
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input);
    input.trim().to_string()
}

pub fn create_area_chart_context(
    opt: usize,
    config: &Configuration,
    appdir: &Path,
    best_date: &str,
) -> Option<ChartTaskContext> {
    if opt >= config.areachartlist.len() {
        return None;
    }
    let chartworkname = config.areachartlist[opt].clone();
    let chartname = chartworkname.clone();
    let charturl = config
        .vfrindividualtemplate
        .replace("<chartdate>", best_date)
        .replace("<charttype>", &chartworkname);
    let clipfolder = appdir.join("clipshapes").join("sectional");

    Some(ChartTaskContext::new(
        &appdir.join("workarea"),
        chartworkname,
        chartname,
        charturl,
        clipfolder,
        false,
        best_date.to_string(),
    ))
}

pub fn create_full_chart_context(
    opt: usize,
    config: &Configuration,
    appdir: &Path,
    best_date: &str,
) -> Option<ChartTaskContext> {
    if opt >= config.fullchartlist.len() {
        return None;
    }
    let entry = &config.fullchartlist[opt];
    let chartworkname = entry[0].clone();
    let charttype = entry[1].clone();
    let chartdisplayname = entry[2].clone();

    let is_ifr = charttype.to_lowercase() == "ifr";
    let template = if is_ifr {
        &config.ifrdownloadtemplate
    } else {
        &config.vfrdownloadtemplate
    };

    let charturl = template
        .replace("<chartdate>", best_date)
        .replace("<charttype>", &chartworkname);

    let clip_sub = match chartdisplayname.as_str() {
        "Enroute_Low" => "enroute_low",
        "Enroute_High" => "enroute_high",
        _ => &chartdisplayname.to_lowercase(),
    };
    let clipfolder = appdir.join("clipshapes").join(clip_sub);

    Some(ChartTaskContext::new(
        &appdir.join("workarea"),
        chartworkname,
        chartdisplayname,
        charturl,
        clipfolder,
        is_ifr,
        best_date.to_string(),
    ))
}

pub async fn run_cli_loop(config: &Configuration, appdir: &Path) -> Result<()> {
    let dates_file = appdir.join("chartdates.json");
    let best_date = get_best_chart_date(&dates_file).unwrap_or_else(|_| "01-01-2025".to_string());

    println!("Chartmaker CLI Mode (Rust)");
    println!("Current FAA chart cycle date: {}", best_date);

    loop {
        println!("\nSelect a chart processing option:");
        println!("0 Process single area VFR chart");
        println!("1 Process single full chart");
        println!("2 Process all sectional and TAC area charts");
        println!("3 Process all full charts");
        println!("4 Download all sectional and TAC area charts");
        println!("5 Download all full charts");
        println!("6 Clean work directories");
        println!("7 Exit chartmaker");

        let choice = prompt_line("\nSelect chart processing command (0-7): ");
        match choice.as_str() {
            "0" => {
                println!("\nSelect area chart to process:");
                for (idx, name) in config.areachartlist.iter().enumerate() {
                    println!("{} {}", idx, name.replace('_', " "));
                }
                let start_over_idx = config.areachartlist.len();
                println!("{} Start over", start_over_idx);

                let sub_choice = prompt_line(&format!(
                    "\nSelect an area chart (0-{}) or '{}' to return: ",
                    start_over_idx, start_over_idx
                ));

                if sub_choice.eq_ignore_ascii_case("s")
                    || sub_choice.eq_ignore_ascii_case("start over")
                    || sub_choice == start_over_idx.to_string()
                {
                    continue;
                }

                if let Ok(idx) = sub_choice.parse::<usize>() {
                    if let Some(ctx) = create_area_chart_context(idx, config, appdir, &best_date) {
                        let _ = run_processing(&ctx, config, appdir, None).await;
                    }
                }
            }
            "1" => {
                println!("\nSelect full chart to process:");
                for (idx, entry) in config.fullchartlist.iter().enumerate() {
                    println!("{} {}", idx, entry[2].replace('_', " "));
                }
                let start_over_idx = config.fullchartlist.len();
                println!("{} Start over", start_over_idx);

                let sub_choice = prompt_line(&format!(
                    "\nSelect a full chart (0-{}) or '{}' to return: ",
                    start_over_idx, start_over_idx
                ));

                if sub_choice.eq_ignore_ascii_case("s")
                    || sub_choice.eq_ignore_ascii_case("start over")
                    || sub_choice == start_over_idx.to_string()
                {
                    continue;
                }

                if let Ok(idx) = sub_choice.parse::<usize>() {
                    if let Some(ctx) = create_full_chart_context(idx, config, appdir, &best_date) {
                        let _ = run_processing(&ctx, config, appdir, None).await;
                    }
                }
            }
            "2" => {
                println!("\nProcessing all area charts...");
                for idx in 0..config.areachartlist.len() {
                    if let Some(ctx) = create_area_chart_context(idx, config, appdir, &best_date) {
                        let _ = run_processing(&ctx, config, appdir, None).await;
                    }
                }
            }
            "3" => {
                println!("\nProcessing all full charts...");
                for idx in 0..config.fullchartlist.len() {
                    if let Some(ctx) = create_full_chart_context(idx, config, appdir, &best_date) {
                        let _ = run_processing(&ctx, config, appdir, None).await;
                    }
                }
            }
            "6" => {
                let workarea = appdir.join("workarea");
                if workarea.exists() {
                    println!("Cleaning workarea: {:?}", workarea);
                    let _ = std::fs::remove_dir_all(&workarea);
                    let _ = std::fs::create_dir_all(&workarea);
                }
            }
            "7" | "exit" | "q" => {
                println!("Exiting chartmaker.");
                break;
            }
            _ => {
                println!("Invalid selection, please try again.");
            }
        }
    }

    Ok(())
}

use chrono::Utc;
use serde_json::json;
use std::time::Instant;

use crate::cli::menu::{create_area_chart_context, create_full_chart_context};
use crate::models::app_message::AppMessage;
use crate::pipeline::runner::{format_duration, run_processing};
use crate::queue::job_queue::CompletedItem;
use crate::server::state::AppState;

/// Background task that continuously polls and executes jobs from the FIFO JobQueue
pub async fn start_queue_worker(state: AppState) {
    loop {
        let next_job = {
            let mut q = state.queue.lock().await;
            if q.current_job.is_none() && !q.queue.is_empty() {
                let mut job = q.queue.remove(0);
                job.status = "running".to_string();
                job.started_at = Some(Utc::now());
                if !job.uid.is_empty() {
                    q.user_jobs.insert(job.uid.clone(), job.clone());
                }
                q.current_job = Some(job.clone());
                Some(job)
            } else {
                None
            }
        };

        if let Some(mut job) = next_job {
            let start_time = Instant::now();
            let uid = job.uid.clone();
            let cancel_token = job.cancel_token.clone();

            let mut msg_resp = AppMessage::new(AppMessage::MT_COMMAND_RESPONSE);
            msg_resp.uid = uid.clone();
            msg_resp.payload = json!("success");
            state.send_to_clients(&uid, msg_resp);

            let mut cancelled = false;

            for (i, item) in job.commandlist.iter().enumerate() {
                if cancel_token.is_cancelled() {
                    cancelled = true;
                    break;
                }

                let cmd_idx: i32 = item.command.parse().unwrap_or(-1);

                let (chart_name, db_filename, maybe_ctx) = match cmd_idx {
                    0 => {
                        let opt_idx: usize = item.chart.parse().unwrap_or_else(|_| {
                            state
                                .config
                                .areachartlist
                                .iter()
                                .position(|c| {
                                    c.eq_ignore_ascii_case(&item.chart)
                                        || c.eq_ignore_ascii_case(&item.chartname)
                                })
                                .unwrap_or(999)
                        });
                        let name = state
                            .config
                            .areachartlist
                            .get(opt_idx)
                            .cloned()
                            .unwrap_or_else(|| {
                                if !item.chartname.is_empty() {
                                    item.chartname.clone()
                                } else {
                                    "Area".to_string()
                                }
                            });
                        let db = format!("{}.mbtiles", name.to_lowercase().replace(' ', "_"));
                        let ctx = create_area_chart_context(
                            opt_idx,
                            &state.config,
                            &state.appdir,
                            &state.queue.lock().await.chartdate,
                        );
                        (name, db, ctx)
                    }
                    1 => {
                        let opt_idx: usize = item.chart.parse().unwrap_or_else(|_| {
                            state
                                .config
                                .fullchartlist
                                .iter()
                                .position(|c| {
                                    c[0].eq_ignore_ascii_case(&item.chart)
                                        || c[0].eq_ignore_ascii_case(&item.chartname)
                                        || (c.len() > 2
                                            && (c[2].eq_ignore_ascii_case(&item.chart)
                                                || c[2].eq_ignore_ascii_case(&item.chartname)))
                                })
                                .unwrap_or(999)
                        });
                        let entry = state.config.fullchartlist.get(opt_idx);
                        let name = entry
                            .map(|e| e[2].replace('_', " "))
                            .unwrap_or_else(|| {
                                if !item.chartname.is_empty() {
                                    item.chartname.clone()
                                } else {
                                    "Full".to_string()
                                }
                            });
                        let db = format!("{}.mbtiles", name.to_lowercase().replace(' ', "_"));
                        let ctx = create_full_chart_context(
                            opt_idx,
                            &state.config,
                            &state.appdir,
                            &state.queue.lock().await.chartdate,
                        );
                        (name, db, ctx)
                    }
                    _ => (String::new(), String::new(), None),
                };

                if let Some(ctx) = maybe_ctx {
                    {
                        let mut q = state.queue.lock().await;
                        q.current_chart = chart_name.clone();
                        if let Some(ref mut cj) = q.current_job {
                            cj.current_chart = chart_name.clone();
                            cj.current_row_index = i as i32;
                        }
                    }

                    let mut rmt = AppMessage::new(AppMessage::MT_RUNNING);
                    rmt.rowindex = i as i32;
                    rmt.filename = chart_name.clone();
                    rmt.dbfilename = db_filename.clone();
                    state.send_to_clients(&uid, rmt);

                    match run_processing(&ctx, &state.config, &state.appdir, Some(&cancel_token)).await
                    {
                        Ok(time_str) => {
                            let mut cmt = AppMessage::new(AppMessage::MT_COMPLETE);
                            cmt.rowindex = i as i32;
                            cmt.payload = json!(time_str);
                            cmt.filename = chart_name.clone();
                            cmt.dbfilename = db_filename.clone();

                            let comp_item = CompletedItem {
                                msg_type: AppMessage::MT_COMPLETE.to_string(),
                                rowindex: i as i32,
                                filename: chart_name,
                                dbfilename: db_filename,
                                payload: time_str,
                            };
                            job.completed_items.push(comp_item);

                            state.send_to_clients(&uid, cmt);
                        }
                        Err(e) => {
                            if cancel_token.is_cancelled() {
                                cancelled = true;
                                break;
                            } else {
                                eprintln!("Error processing chart {}: {:?}", chart_name, e);
                            }
                        }
                    }
                }
            }

            let total_dur = format_duration(start_time.elapsed());
            {
                let mut q = state.queue.lock().await;
                if cancelled {
                    job.status = "cancelled".to_string();
                } else {
                    job.status = "completed".to_string();
                    job.completed_at = Some(Utc::now());
                    job.total_time = total_dur.clone();

                    let mut tmsg = AppMessage::new(AppMessage::MT_TIMING);
                    tmsg.payload = json!(total_dur);
                    state.send_to_clients(&uid, tmsg);
                }
                if !uid.is_empty() {
                    q.user_jobs.insert(uid.clone(), job);
                }
                q.current_job = None;
                q.current_chart.clear();
            }
        } else {
            tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
        }
    }
}

//! Binary DVD-Video IFO (`VIDEO_TS.IFO` and `VTS_xx_0.IFO`) structure parser.
//!
//! Provides sub-10ms inspection of DVD titles, Program Chain (`VTS_PGCITI`) durations,
//! `VTS_PTT_SRPT` title-to-PGC mappings, cell playback sector topologies (`C_PBIT`),
//! and audio/subtitle/chapter metadata without spawning FFmpeg subprocesses.

use std::path::Path;

use crate::domain::entities::disc::{
    AudioStreamInfo, Chapter, SubtitleStreamInfo, TitleStreamDetails,
};
use crate::domain::heuristics::TitleCellTopology;
use crate::ffmpeg::DvdTitleInfo;

/// Resolves a 1-based VTS title number (`vts_ttn`) to its 1-based Program Chain Number (`pgcn`)
/// via the `VTS_PTT_SRPT` table (sector pointer at `0xC8..0xCC` in `VTS_xx_0.IFO`).
/// Falls back to `vts_ttn as usize` if `VTS_PTT_SRPT` is absent or out of bounds.
pub fn resolve_vts_ttn_to_pgcn(vts_data: &[u8], vts_ttn: u8) -> usize {
    let fallback = (vts_ttn as usize).max(1);
    if vts_ttn == 0 || vts_data.len() < 0xCC {
        return fallback;
    }
    let ptt_srpt_sec = u32::from_be_bytes([
        vts_data[0xC8],
        vts_data[0xC9],
        vts_data[0xCA],
        vts_data[0xCB],
    ]) as usize;
    if ptt_srpt_sec == 0 {
        return fallback;
    }
    let ptt_srpt_off = ptt_srpt_sec * 2048;
    if ptt_srpt_off + 8 > vts_data.len() {
        return fallback;
    }
    let nr_ttu = u16::from_be_bytes([vts_data[ptt_srpt_off], vts_data[ptt_srpt_off + 1]]) as usize;
    let ttn_idx = (vts_ttn as usize).saturating_sub(1);
    if nr_ttu == 0 || ttn_idx >= nr_ttu || ptt_srpt_off + 8 + nr_ttu * 4 > vts_data.len() {
        return fallback;
    }
    let ptr_off = ptt_srpt_off + 8 + ttn_idx * 4;
    let ttu_rel = u32::from_be_bytes([
        vts_data[ptr_off],
        vts_data[ptr_off + 1],
        vts_data[ptr_off + 2],
        vts_data[ptr_off + 3],
    ]) as usize;
    let first_ptt_off = ptt_srpt_off + ttu_rel;
    if ttu_rel < 8 || first_ptt_off + 4 > vts_data.len() {
        return fallback;
    }
    let pgcn =
        u16::from_be_bytes([vts_data[first_ptt_off], vts_data[first_ptt_off + 1]]) as usize;
    if pgcn == 0 { fallback } else { pgcn }
}

/// Ultra-fast DVD title probing by directly reading DVD-Video IFO structures (`VIDEO_TS.IFO` and `VTS_xx_0.IFO`).
/// Returns titles and exact durations in <10ms without launching FFmpeg subprocesses.
pub fn probe_dvd_titles_ifo(dvd_path: &Path) -> Option<Vec<DvdTitleInfo>> {
    let candidates = [
        dvd_path.join("VIDEO_TS").join("VIDEO_TS.IFO"),
        dvd_path.join("video_ts").join("video_ts.ifo"),
        dvd_path.join("VIDEO_TS").join("video_ts.ifo"),
        dvd_path.join("video_ts").join("VIDEO_TS.IFO"),
        dvd_path.join("VIDEO_TS.IFO"),
        dvd_path.join("video_ts.ifo"),
    ];

    let ifo_path = candidates.into_iter().find(|p| p.exists())?;
    let data = std::fs::read(&ifo_path).ok()?;
    if data.len() < 0xC8 {
        return None;
    }

    let sector_offset = u32::from_be_bytes([data[0xc4], data[0xc5], data[0xc6], data[0xc7]]);
    let byte_offset = (sector_offset as usize) * 2048;
    if byte_offset + 8 > data.len() {
        return None;
    }

    let nr_titles = u16::from_be_bytes([data[byte_offset], data[byte_offset + 1]]) as usize;
    if nr_titles == 0 || byte_offset + 8 + nr_titles * 12 > data.len() {
        return None;
    }

    let ifo_dir = ifo_path.parent().unwrap_or(dvd_path);
    let mut titles = Vec::with_capacity(nr_titles);

    for t in 0..nr_titles {
        let entry_off = byte_offset + 8 + t * 12;
        let vtsn = data[entry_off + 6];
        let vts_ttn = data[entry_off + 7];

        let mut duration_secs = 0.0;
        let vts_candidates = [
            ifo_dir.join(format!("VTS_{:02}_0.IFO", vtsn)),
            ifo_dir.join(format!("vts_{:02}_0.ifo", vtsn)),
            ifo_dir.join(format!("VTS_{:02}_0.ifo", vtsn)),
        ];

        if let Some(vts_path) = vts_candidates.into_iter().find(|p| p.exists())
            && let Ok(vts_data) = std::fs::read(&vts_path)
            && vts_data.len() > 0xCC + 4
        {
            let pgciti_sec = u32::from_be_bytes([
                vts_data[0xCC],
                vts_data[0xCD],
                vts_data[0xCE],
                vts_data[0xCF],
            ]);
            let pgciti_off = (pgciti_sec as usize) * 2048;
            let pgcn = resolve_vts_ttn_to_pgcn(&vts_data, vts_ttn);
            if vts_ttn > 0 && pgciti_off + 8 + pgcn * 8 <= vts_data.len() {
                let pgc_entry_off = pgciti_off + 8 + (pgcn - 1) * 8;
                let pgc_start_rel = u32::from_be_bytes([
                    vts_data[pgc_entry_off + 4],
                    vts_data[pgc_entry_off + 5],
                    vts_data[pgc_entry_off + 6],
                    vts_data[pgc_entry_off + 7],
                ]);
                let pgc_off = pgciti_off + (pgc_start_rel as usize);
                if pgc_off + 8 <= vts_data.len() {
                    let h_bcd = vts_data[pgc_off + 4];
                    let m_bcd = vts_data[pgc_off + 5];
                    let s_bcd = vts_data[pgc_off + 6];
                    let h = (((h_bcd >> 4) * 10) + (h_bcd & 0x0F)) as f64;
                    let m = (((m_bcd >> 4) * 10) + (m_bcd & 0x0F)) as f64;
                    let s = (((s_bcd >> 4) * 10) + (s_bcd & 0x0F)) as f64;
                    duration_secs = h * 3600.0 + m * 60.0 + s;
                }
            }
        }

        titles.push(DvdTitleInfo {
            title_num: (t + 1) as u32,
            duration_secs,
        });
    }

    if titles.is_empty() {
        None
    } else {
        Some(titles)
    }
}

/// Parses binary `VIDEO_TS.IFO` and `VTS_xx_0.IFO` Cell Playback Tables (`C_PBIT`) to build
/// [`TitleCellTopology`] entries for structural fake-title de-obfuscation.
pub fn probe_dvd_cell_topologies_ifo(dvd_path: &Path) -> Option<Vec<TitleCellTopology>> {
    let candidates = [
        dvd_path.join("VIDEO_TS").join("VIDEO_TS.IFO"),
        dvd_path.join("video_ts").join("video_ts.ifo"),
        dvd_path.join("VIDEO_TS").join("video_ts.ifo"),
        dvd_path.join("video_ts").join("VIDEO_TS.IFO"),
        dvd_path.join("VIDEO_TS.IFO"),
        dvd_path.join("video_ts.ifo"),
    ];

    let ifo_path = candidates.into_iter().find(|p| p.exists())?;
    let data = std::fs::read(&ifo_path).ok()?;
    if data.len() < 0xC8 {
        return None;
    }

    let sector_offset = u32::from_be_bytes([data[0xc4], data[0xc5], data[0xc6], data[0xc7]]);
    let byte_offset = (sector_offset as usize) * 2048;
    if byte_offset + 8 > data.len() {
        return None;
    }

    let nr_titles = u16::from_be_bytes([data[byte_offset], data[byte_offset + 1]]) as usize;
    if nr_titles == 0 || byte_offset + 8 + nr_titles * 12 > data.len() {
        return None;
    }

    let ifo_dir = ifo_path.parent().unwrap_or(dvd_path);
    let mut topologies = Vec::with_capacity(nr_titles);

    for t in 0..nr_titles {
        let entry_off = byte_offset + 8 + t * 12;
        let angle_count = data[entry_off + 1].max(1);
        let vtsn = data[entry_off + 6];
        let vts_ttn = data[entry_off + 7];

        let mut duration_secs = 0.0;
        let mut cell_sectors = Vec::new();
        let vts_candidates = [
            ifo_dir.join(format!("VTS_{:02}_0.IFO", vtsn)),
            ifo_dir.join(format!("vts_{:02}_0.ifo", vtsn)),
            ifo_dir.join(format!("VTS_{:02}_0.ifo", vtsn)),
        ];

        if let Some(vts_path) = vts_candidates.into_iter().find(|p| p.exists())
            && let Ok(vts_data) = std::fs::read(&vts_path)
            && vts_data.len() > 0xCC + 4
        {
            let pgciti_sec = u32::from_be_bytes([
                vts_data[0xCC],
                vts_data[0xCD],
                vts_data[0xCE],
                vts_data[0xCF],
            ]);
            let pgciti_off = (pgciti_sec as usize) * 2048;
            let pgcn = resolve_vts_ttn_to_pgcn(&vts_data, vts_ttn);
            if vts_ttn > 0 && pgciti_off + 8 + pgcn * 8 <= vts_data.len() {
                let pgc_entry_off = pgciti_off + 8 + (pgcn - 1) * 8;
                let pgc_start_rel = u32::from_be_bytes([
                    vts_data[pgc_entry_off + 4],
                    vts_data[pgc_entry_off + 5],
                    vts_data[pgc_entry_off + 6],
                    vts_data[pgc_entry_off + 7],
                ]);
                let pgc_off = pgciti_off + (pgc_start_rel as usize);
                if pgc_off + 0xEC <= vts_data.len() {
                    let nr_cells = vts_data[pgc_off + 3] as usize;
                    let h_bcd = vts_data[pgc_off + 4];
                    let m_bcd = vts_data[pgc_off + 5];
                    let s_bcd = vts_data[pgc_off + 6];
                    let h = (((h_bcd >> 4) * 10) + (h_bcd & 0x0F)) as f64;
                    let m = (((m_bcd >> 4) * 10) + (m_bcd & 0x0F)) as f64;
                    let s = (((s_bcd >> 4) * 10) + (s_bcd & 0x0F)) as f64;
                    duration_secs = h * 3600.0 + m * 60.0 + s;

                    let c_pbit_rel =
                        u16::from_be_bytes([vts_data[pgc_off + 0xE8], vts_data[pgc_off + 0xE9]])
                            as usize;
                    let c_pbit_off = pgc_off + c_pbit_rel;
                    if c_pbit_rel > 0
                        && nr_cells > 0
                        && c_pbit_off + nr_cells * 24 <= vts_data.len()
                    {
                        for c in 0..nr_cells {
                            let c_off = c_pbit_off + c * 24;
                            let first_sec = u32::from_be_bytes([
                                vts_data[c_off + 8],
                                vts_data[c_off + 9],
                                vts_data[c_off + 10],
                                vts_data[c_off + 11],
                            ]);
                            let last_sec = u32::from_be_bytes([
                                vts_data[c_off + 16],
                                vts_data[c_off + 17],
                                vts_data[c_off + 18],
                                vts_data[c_off + 19],
                            ]);
                            cell_sectors.push((first_sec, last_sec));
                        }
                    }
                }
            }
        }

        topologies.push(TitleCellTopology {
            title_num: (t + 1) as u32,
            duration_secs,
            cell_sectors,
            angle_count,
        });
    }

    if topologies.is_empty() {
        None
    } else {
        Some(topologies)
    }
}

/// Directly parses binary DVD-Video `VIDEO_TS.IFO` and `VTS_xx_0.IFO` tables to extract deep per-title stream topology:
/// chapters, audio streams (codec, language, channels), subtitle tracks, video aspect/resolution, and camera angles.
pub fn probe_title_details_ifo(dvd_path: &Path, title_num: u32) -> Option<TitleStreamDetails> {
    if title_num == 0 {
        return None;
    }

    let candidates = [
        dvd_path.join("VIDEO_TS").join("VIDEO_TS.IFO"),
        dvd_path.join("video_ts").join("video_ts.ifo"),
        dvd_path.join("VIDEO_TS").join("video_ts.ifo"),
        dvd_path.join("video_ts").join("VIDEO_TS.IFO"),
        dvd_path.join("VIDEO_TS.IFO"),
        dvd_path.join("video_ts.ifo"),
    ];

    let ifo_path = candidates.into_iter().find(|p| p.exists())?;
    let data = std::fs::read(&ifo_path).ok()?;
    if data.len() < 0xC8 {
        return None;
    }

    let sector_offset = u32::from_be_bytes([data[0xc4], data[0xc5], data[0xc6], data[0xc7]]);
    let byte_offset = (sector_offset as usize) * 2048;
    if byte_offset + 8 > data.len() {
        return None;
    }

    let nr_titles = u16::from_be_bytes([data[byte_offset], data[byte_offset + 1]]) as usize;
    let idx = (title_num as usize).checked_sub(1)?;
    if idx >= nr_titles || byte_offset + 8 + nr_titles * 12 > data.len() {
        return None;
    }

    let entry_off = byte_offset + 8 + idx * 12;
    let angles = data[entry_off + 1].max(1);
    let num_chapters = u16::from_be_bytes([data[entry_off + 2], data[entry_off + 3]]) as u32;
    let vtsn = data[entry_off + 6];
    let vts_ttn = data[entry_off + 7];

    let ifo_dir = ifo_path.parent().unwrap_or(dvd_path);
    let vts_candidates = [
        ifo_dir.join(format!("VTS_{:02}_0.IFO", vtsn)),
        ifo_dir.join(format!("vts_{:02}_0.ifo", vtsn)),
        ifo_dir.join(format!("VTS_{:02}_0.ifo", vtsn)),
    ];

    let vts_path = vts_candidates.into_iter().find(|p| p.exists())?;
    let vts_data = std::fs::read(&vts_path).ok()?;
    if vts_data.len() < 0x300 {
        return None;
    }

    // 1. Video attributes at 0x200..0x202
    let v_attr0 = vts_data[0x200];
    let is_pal = ((v_attr0 >> 4) & 0x03) == 1;
    let aspect_code = (v_attr0 >> 2) & 0x03;
    let aspect_ratio = Some(if aspect_code == 3 {
        "16:9".to_string()
    } else {
        "4:3".to_string()
    });
    let resolution = Some(if is_pal { (720, 576) } else { (720, 480) });
    let frame_rate = Some(if is_pal {
        "25.00".to_string()
    } else {
        "29.97".to_string()
    });

    // 2. Audio streams at 0x202..0x204 (count) and 0x204.. (8 bytes per stream, up to 8 streams)
    let nr_audio = (u16::from_be_bytes([vts_data[0x202], vts_data[0x203]]) as usize).min(8);
    let mut audio_streams = Vec::with_capacity(nr_audio);
    for a in 0..nr_audio {
        let a_off = 0x204 + a * 8;
        if a_off + 8 > vts_data.len() {
            break;
        }
        let coding_mode = (vts_data[a_off] >> 5) & 0x07;
        let codec = match coding_mode {
            0 => "ac3",
            2 | 3 => "mp2",
            4 => "pcm",
            6 => "dts",
            _ => "ac3",
        }
        .to_string();
        let channels = ((vts_data[a_off + 1] & 0x07) + 1) as u32;
        let lang_bytes = &vts_data[a_off + 2..a_off + 4];
        let language = if lang_bytes.iter().all(|b| b.is_ascii_alphabetic()) {
            String::from_utf8_lossy(lang_bytes).to_lowercase()
        } else {
            "und".to_string()
        };
        let channel_layout = match channels {
            6 => "5.1".to_string(),
            2 => "2.0".to_string(),
            1 => "1.0".to_string(),
            c => format!("{}ch", c),
        };
        let ext_code = vts_data[a_off + 5];
        let title = match ext_code {
            2 => Some("Visually Impaired".to_string()),
            3 | 4 => Some("Director's Commentary".to_string()),
            _ => None,
        };
        audio_streams.push(AudioStreamInfo {
            stream_index: a as u32,
            codec,
            language,
            channels,
            channel_layout,
            title,
        });
    }

    // 3. Subtitle streams at 0x254..0x256 (count) and 0x256.. (6 bytes per stream, up to 32 streams)
    let nr_subs = (u16::from_be_bytes([vts_data[0x254], vts_data[0x255]]) as usize).min(32);
    let mut subtitle_streams = Vec::with_capacity(nr_subs);
    for s in 0..nr_subs {
        let s_off = 0x256 + s * 6;
        if s_off + 6 > vts_data.len() {
            break;
        }
        let lang_bytes = &vts_data[s_off + 2..s_off + 4];
        let language = if lang_bytes.iter().all(|b| b.is_ascii_alphabetic()) {
            String::from_utf8_lossy(lang_bytes).to_lowercase()
        } else {
            "und".to_string()
        };
        let ext_code = vts_data[s_off + 5];
        let forced = ext_code == 9;
        subtitle_streams.push(SubtitleStreamInfo {
            stream_index: s as u32,
            language,
            format: "dvdsub".to_string(),
            forced,
        });
    }

    // 4. PGC Duration & Chapters
    let mut duration_secs = 0.0;
    let mut chapters = Vec::new();
    let pgciti_sec = u32::from_be_bytes([
        vts_data[0xCC],
        vts_data[0xCD],
        vts_data[0xCE],
        vts_data[0xCF],
    ]);
    let pgciti_off = (pgciti_sec as usize) * 2048;
    let pgcn = resolve_vts_ttn_to_pgcn(&vts_data, vts_ttn);
    if vts_ttn > 0 && pgciti_off + 8 + pgcn * 8 <= vts_data.len() {
        let pgc_entry_off = pgciti_off + 8 + (pgcn - 1) * 8;
        let pgc_start_rel = u32::from_be_bytes([
            vts_data[pgc_entry_off + 4],
            vts_data[pgc_entry_off + 5],
            vts_data[pgc_entry_off + 6],
            vts_data[pgc_entry_off + 7],
        ]);
        let pgc_off = pgciti_off + (pgc_start_rel as usize);
        if pgc_off + 0xEC <= vts_data.len() {
            let nr_programs = vts_data[pgc_off + 2] as u32;
            let nr_cells = vts_data[pgc_off + 3] as usize;
            let h_bcd = vts_data[pgc_off + 4];
            let m_bcd = vts_data[pgc_off + 5];
            let s_bcd = vts_data[pgc_off + 6];
            let h = (((h_bcd >> 4) * 10) + (h_bcd & 0x0F)) as f64;
            let m = (((m_bcd >> 4) * 10) + (m_bcd & 0x0F)) as f64;
            let s = (((s_bcd >> 4) * 10) + (s_bcd & 0x0F)) as f64;
            duration_secs = h * 3600.0 + m * 60.0 + s;

            let pg_map_rel =
                u16::from_be_bytes([vts_data[pgc_off + 0xE6], vts_data[pgc_off + 0xE7]]) as usize;
            let c_pbit_rel =
                u16::from_be_bytes([vts_data[pgc_off + 0xE8], vts_data[pgc_off + 0xE9]]) as usize;
            let c_pbit_off = pgc_off + c_pbit_rel;
            if c_pbit_rel > 0 && nr_cells > 0 && c_pbit_off + nr_cells * 24 <= vts_data.len() {
                let mut cell_durations = Vec::with_capacity(nr_cells);
                for c in 0..nr_cells {
                    let cell_off = c_pbit_off + c * 24;
                    let ch_bcd = vts_data[cell_off + 4];
                    let cm_bcd = vts_data[cell_off + 5];
                    let cs_bcd = vts_data[cell_off + 6];
                    let ch_h = (((ch_bcd >> 4) * 10) + (ch_bcd & 0x0F)) as f64;
                    let ch_m = (((cm_bcd >> 4) * 10) + (cm_bcd & 0x0F)) as f64;
                    let ch_s = (((cs_bcd >> 4) * 10) + (cs_bcd & 0x0F)) as f64;
                    cell_durations.push(ch_h * 3600.0 + ch_m * 60.0 + ch_s);
                }

                let pg_map_off = pgc_off + pg_map_rel;
                let has_valid_pg_map = pg_map_rel > 0
                    && nr_programs > 0
                    && pg_map_off + (nr_programs as usize) <= vts_data.len()
                    && (0..nr_programs as usize).all(|p| {
                        let start_cell = vts_data[pg_map_off + p] as usize;
                        start_cell >= 1 && start_cell <= nr_cells
                    });

                let mut cur_start = 0.0;
                if has_valid_pg_map {
                    let prog_count = nr_programs as usize;
                    for p in 0..prog_count {
                        let start_cell_idx = (vts_data[pg_map_off + p] as usize) - 1;
                        let end_cell_idx = if p + 1 < prog_count {
                            ((vts_data[pg_map_off + p + 1] as usize) - 1).max(start_cell_idx + 1)
                        } else {
                            nr_cells
                        }
                        .min(nr_cells);
                        let prog_dur: f64 = cell_durations[start_cell_idx..end_cell_idx]
                            .iter()
                            .copied()
                            .sum();
                        if prog_dur > 0.0 {
                            let ch_idx = (chapters.len() + 1) as u32;
                            chapters.push(Chapter {
                                index: ch_idx,
                                start_secs: cur_start,
                                end_secs: cur_start + prog_dur,
                                title: format!("Chapter {:02}", ch_idx),
                            });
                            cur_start += prog_dur;
                        }
                    }
                } else {
                    for cell_dur in cell_durations {
                        if cell_dur > 0.0 {
                            let ch_idx = (chapters.len() + 1) as u32;
                            chapters.push(Chapter {
                                index: ch_idx,
                                start_secs: cur_start,
                                end_secs: cur_start + cell_dur,
                                title: format!("Chapter {:02}", ch_idx),
                            });
                            cur_start += cell_dur;
                        }
                    }
                }
            } else {
                let count = nr_programs.max(num_chapters).max(1);
                let step = if duration_secs > 0.0 {
                    duration_secs / (count as f64)
                } else {
                    300.0
                };
                for c in 1..=count {
                    chapters.push(Chapter {
                        index: c,
                        start_secs: (c - 1) as f64 * step,
                        end_secs: c as f64 * step,
                        title: format!("Chapter {:02}", c),
                    });
                }
            }
        }
    }

    Some(TitleStreamDetails {
        title_num,
        duration_secs,
        chapters,
        audio_streams,
        subtitle_streams,
        angles,
        resolution,
        aspect_ratio,
        frame_rate,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ifo_parser_resolve_vts_ttn_fallbacks() {
        assert_eq!(resolve_vts_ttn_to_pgcn(&[], 0), 1);
        assert_eq!(resolve_vts_ttn_to_pgcn(&[0u8; 10], 2), 2);

        // Valid data length but zero ptt_srpt_sec
        let mut data = vec![0u8; 300];
        assert_eq!(resolve_vts_ttn_to_pgcn(&data, 3), 3);

        // ptt_srpt_sec points past data length
        data[0xC8..0xCC].copy_from_slice(&100u32.to_be_bytes());
        assert_eq!(resolve_vts_ttn_to_pgcn(&data, 4), 4);
    }

    #[test]
    fn test_ifo_parser_probe_nonexistent_paths() {
        let fake_path = Path::new("Z:\\nonexistent_dvd_path_xyz");
        assert!(probe_dvd_titles_ifo(fake_path).is_none());
        assert!(probe_dvd_cell_topologies_ifo(fake_path).is_none());
        assert!(probe_title_details_ifo(fake_path, 0).is_none());
        assert!(probe_title_details_ifo(fake_path, 1).is_none());
    }

    #[test]
    fn test_ifo_parser_synthetic_full_parsing() {
        let tmp = std::env::temp_dir().join(format!("ifo_test_synth_{}", std::process::id()));
        let video_ts = tmp.join("VIDEO_TS");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&video_ts).unwrap();

        // 1. VIDEO_TS.IFO
        let mut vmg = vec![0u8; 4096];
        vmg[0xc4..0xc8].copy_from_slice(&1u32.to_be_bytes());
        vmg[2048..2050].copy_from_slice(&1u16.to_be_bytes()); // 1 title
        let entry = 2048 + 8;
        vmg[entry + 1] = 2; // 2 angles
        vmg[entry + 2] = 0;
        vmg[entry + 3] = 2; // 2 chapters
        vmg[entry + 6] = 1; // VTS 1
        vmg[entry + 7] = 1; // TTN 1
        std::fs::write(video_ts.join("VIDEO_TS.IFO"), &vmg).unwrap();

        // 2. VTS_01_0.IFO
        let mut vts = vec![0u8; 8192];
        // Attributes
        vts[0x200] = 0x10 | 0x0C; // PAL (0x10), 16:9 (0x0C)
        vts[0x202..0x204].copy_from_slice(&1u16.to_be_bytes()); // 1 audio stream
        vts[0x204] = 0x00; // AC3
        vts[0x205] = 5; // 6 channels (5.1)
        vts[0x206] = b'e';
        vts[0x207] = b'n';

        vts[0x254..0x256].copy_from_slice(&1u16.to_be_bytes()); // 1 sub stream
        vts[0x258] = b'e';
        vts[0x259] = b'n';
        vts[0x25B] = 9; // Forced

        // PTT_SRPT at sector 1 (offset 2048)
        vts[0xC8..0xCC].copy_from_slice(&1u32.to_be_bytes());
        let ptt = 2048;
        vts[ptt..ptt + 2].copy_from_slice(&1u16.to_be_bytes());
        vts[ptt + 8..ptt + 12].copy_from_slice(&12u32.to_be_bytes());
        vts[ptt + 12..ptt + 14].copy_from_slice(&1u16.to_be_bytes()); // PGCN 1

        // PGCITI at sector 2 (offset 4096)
        vts[0xCC..0xD0].copy_from_slice(&2u32.to_be_bytes());
        let pgciti = 4096;
        vts[pgciti..pgciti + 2].copy_from_slice(&1u16.to_be_bytes());
        let ptr = pgciti + 8;
        vts[ptr + 4..ptr + 8].copy_from_slice(&16u32.to_be_bytes());
        let pgc = pgciti + 16;
        vts[pgc + 2] = 2; // 2 programs
        vts[pgc + 3] = 2; // 2 cells
        vts[pgc + 4] = 0x01; // 1 hour
        vts[pgc + 5] = 0x30; // 30 mins
        vts[pgc + 6] = 0x00; // 0 secs

        let pg_map_rel: u16 = 0xEC;
        let c_pbit_rel: u16 = 0xF0;
        vts[pgc + 0xE6..pgc + 0xE8].copy_from_slice(&pg_map_rel.to_be_bytes());
        vts[pgc + 0xE8..pgc + 0xEA].copy_from_slice(&c_pbit_rel.to_be_bytes());
        vts[pgc + 0xEC] = 1;
        vts[pgc + 0xED] = 2;

        let c1 = pgc + (c_pbit_rel as usize);
        vts[c1 + 4] = 0x00;
        vts[c1 + 5] = 0x45; // 45 min
        vts[c1 + 6] = 0x00;
        vts[c1 + 8..c1 + 12].copy_from_slice(&100u32.to_be_bytes());
        vts[c1 + 16..c1 + 20].copy_from_slice(&200u32.to_be_bytes());

        let c2 = c1 + 24;
        vts[c2 + 4] = 0x00;
        vts[c2 + 5] = 0x45; // 45 min
        vts[c2 + 6] = 0x00;
        vts[c2 + 8..c2 + 12].copy_from_slice(&201u32.to_be_bytes());
        vts[c2 + 16..c2 + 20].copy_from_slice(&300u32.to_be_bytes());

        std::fs::write(video_ts.join("VTS_01_0.IFO"), &vts).unwrap();

        let titles = probe_dvd_titles_ifo(&tmp).unwrap();
        assert_eq!(titles.len(), 1);
        assert_eq!(titles[0].duration_secs, 5400.0);

        let topologies = probe_dvd_cell_topologies_ifo(&tmp).unwrap();
        assert_eq!(topologies.len(), 1);
        assert_eq!(topologies[0].angle_count, 2);
        assert_eq!(topologies[0].cell_sectors.len(), 2);

        let details = probe_title_details_ifo(&tmp, 1).unwrap();
        assert_eq!(details.title_num, 1);
        assert_eq!(details.duration_secs, 5400.0);
        assert_eq!(details.resolution, Some((720, 576)));
        assert_eq!(details.aspect_ratio.as_deref(), Some("16:9"));
        assert_eq!(details.audio_streams.len(), 1);
        assert_eq!(details.audio_streams[0].codec, "ac3");
        assert_eq!(details.subtitle_streams.len(), 1);
        assert!(details.subtitle_streams[0].forced);
        assert_eq!(details.chapters.len(), 2);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}


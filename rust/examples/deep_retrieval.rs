// Copyright (c) 2026 vectorless developers
// SPDX-License-Identifier: Apache-2.0

//! Complex retrieval example — forces SubAgent navigation, not fast path.
//!
//! This example indexes a document where the answer to a tricky question
//! is NOT directly accessible via keyword lookup in the ReasoningIndex.
//! The SubAgent must navigate through multiple levels, collect evidence
//! from different sections, and synthesize a cross-referenced answer.
//!
//! # Usage
//!
//! ```bash
//! LLM_API_KEY=sk-xxx LLM_MODEL=gpt-4o \
//!   LLM_ENDPOINT=https://api.openai.com/v1 cargo run --example deep_retrieval
//! ```

use vectorless::{EngineBuilder, IndexContext, IndexOptions, QueryContext};

/// A compact but deeply nested document about a fictional space mission.
///
/// Structure (4 levels deep):
///
/// Mission Atlas Report
/// ├── Launch Operations
/// │   ├── Vehicle Configuration
/// │   │   ├── Stage 1 Parameters
/// │   │   └── Stage 2 Parameters
/// │   └── Countdown Timeline
/// │       ├── T-48h to T-12h
/// │       └── T-12h to T-0
/// ├── Orbital Mechanics
/// │   ├── Transfer Orbit Analysis
/// │   │   ├── Delta-V Budget
/// │   │   └── Gravity Assist Profile
/// │   └── Station-Keeping Schedule
/// ├── Payload Operations
/// │   ├── Satellite Alpha Deployment
/// │   │   ├── Separation Sequence
/// │   │   └── Solar Panel Extension
/// │   ├── Satellite Beta Deployment
/// │   │   ├── Antenna Calibration
/// │   │   └── Frequency Assignment
/// │   └── Re-entry Capsule
/// │       ├── Heat Shield Specs
/// │       └── Landing Zone Selection
/// └── Mission Anomalies
///     ├── Day 3 Communication Blackout
///     └── Day 17 Thruster Misfire
const MISSION_REPORT: &str = r#"
# Mission Atlas Report

## Launch Operations

### Vehicle Configuration

#### Stage 1 Parameters

The first stage utilizes a LOX/RP-1 bipropellant configuration with a sea-level thrust of 7,600 kN. Burn time is 162 seconds with a specific impulse of 282 seconds. The propellant mass fraction is 0.894. Stage separation occurs at T+162s at an altitude of approximately 68 km with a velocity of 2,340 m/s.

#### Stage 2 Parameters

The second stage employs a single RL-10C engine using LOX/LH2 with a vacuum thrust of 110 kN. Burn duration extends to 370 seconds with a specific impulse of 448 seconds. The stage carries 20,800 kg of propellant. Engine ignition occurs at T+165s following a 3-second coast phase after stage separation.

### Countdown Timeline

#### T-48h to T-12h

During the early countdown phase, the launch team completed propellant loading verification and navigation system alignment. A minor issue was detected in the Stage 2 fuel temperature sensor at T-36h, which was resolved by recalibrating the sensor threshold from 20.1K to 19.8K. Weather briefing at T-24h indicated 85% probability of favorable conditions with upper-level winds at 45 knots.

#### T-12h to T-0

Final countdown proceeded nominally. Auxiliary power unit start occurred at T-4h. Range safety checks completed at T-2h. Go/No-Go poll at T-30 minutes was unanimous across all stations. Terminal count at T-9 minutes was initiated with no holds. Liftoff occurred at 14:37:22 UTC on March 15, achieving the targeted azimuth of 72.3 degrees.

## Orbital Mechanics

### Transfer Orbit Analysis

#### Delta-V Budget

The total mission delta-V budget is 4,832 m/s, allocated as follows: ascent to parking orbit 1,890 m/s, trans-target injection 2,210 m/s, orbit insertion 510 m/s, and station-keeping reserve 222 m/s. The parking orbit was achieved at 185 km circular with an inclination of 28.5 degrees. The gravity assist maneuver at Titan contributed an effective delta-V savings of 380 m/s, which allowed the mission to carry 15% more payload than the original baseline design.

#### Gravity Assist Profile

The Titan flyby occurred on Day 47 at a closest approach distance of 950 km. The bending angle was 38.7 degrees with an asymptotic velocity of 4.2 km/s relative to Titan. This maneuver shifted the spacecraft trajectory from a Hohmann-type direct transfer to a gravity-assisted trajectory, reducing total flight time from 187 days to 143 days. Post-flyby trajectory correction burn of 3.4 m/s was executed on Day 49 to refine the approach corridor.

### Station-Keeping Schedule

Station-keeping maneuvers are planned at 14-day intervals with a delta-V allocation of 2.8 m/s per maneuver. The first three maneuvers consumed 2.6, 3.1, and 2.5 m/s respectively, staying within the allocated budget. Orbital decay rate without correction is approximately 0.3 km per 14-day cycle due to atmospheric drag at the operational altitude of 420 km.

## Payload Operations

### Satellite Alpha Deployment

#### Separation Sequence

Satellite Alpha separated from the payload adapter at T+3h42m using a Marman band release mechanism. Separation velocity was 0.45 m/s with a tip-off rate of 0.02 deg/s. Initial telemetry confirmed solar panel deployment signal at T+3h58m. First ground station contact occurred over Svalbard at T+4h12m confirming nominal spacecraft health.

#### Solar Panel Extension

Both solar arrays deployed fully within 8 minutes of the deployment command. Array 1 generated 4,280 W and Array 2 generated 4,310 W, for a combined initial output of 8,590 W against a design target of 8,400 W. The arrays use triple-junction GaAs cells with a beginning-of-life efficiency of 30.7%. Power margin at end-of-life (7 years) is projected at 6,950 W, still above the minimum operational requirement of 6,200 W.

### Satellite Beta Deployment

#### Antenna Calibration

Satellite Beta's high-gain antenna completed calibration in three phases. Phase 1 (boresight alignment) achieved a pointing accuracy of 0.023 degrees against a requirement of 0.05 degrees. Phase 2 (pattern verification) confirmed the sidelobe levels were within specification at -28 dB below main beam. Phase 3 (EIRP verification) measured 52.4 dBW against a required minimum of 51.0 dBW.

#### Frequency Assignment

Satellite Beta operates in Ka-band with a downlink center frequency of 20.185 GHz and an uplink at 30.050 GHz. The allocated bandwidth is 500 MHz per polarization, supporting 24 transponders with 36 MHz spacing. Cross-polarization isolation exceeds 30 dB. The link budget supports a minimum data rate of 1.2 Gbps under rain fade conditions corresponding to 99.7% availability in the primary coverage zone.

### Re-entry Capsule

#### Heat Shield Specs

The re-entry capsule thermal protection system uses a phenolic-impregnated carbon ablator (PICA-X) with a thickness of 33 mm on the forebody. Maximum predicted heat flux is 185 W/cm² at the stagnation point during re-entry at 11.2 km/s. The heat shield mass is 86 kg, representing 12% of the total capsule dry mass of 717 kg. The backshell uses a lighter SLA-561V material with a 15 mm thickness rated for 45 W/cm².

#### Landing Zone Selection

The primary landing zone is located at 34.2°N 108.7°W in the White Sands Proving Ground, with an elliptical footprint of 15 km × 8 km at the 3-sigma confidence level. Wind drift analysis based on 10 years of upper-atmosphere data predicts a mean offset of 3.2 km northeast. The backup landing zone is at 32.5°N 106.5°W near Fort Bliss, activated only if the primary zone weather violates the surface wind constraint of 12 m/s.

## Mission Anomalies

### Day 3 Communication Blackout

At approximately 07:14 UTC on Day 3, the primary S-band transponder experienced an unexpected carrier loss lasting 4 hours and 22 minutes. Root cause analysis identified a single-event upset (SEU) in the command decoder ASIC, caused by a high-energy proton from the inner Van Allen belt. The transponder recovered autonomously after a watchdog timer reset. No command sequences were lost as the onboard computer continued executing the stored timeline. Redundant transponder was not activated because the primary recovery occurred before the 6-hour switchover threshold.

### Day 17 Thruster Misfire

At 14:52 UTC on Day 17, thruster cluster B3 (one of eight attitude control clusters) fired for 2.3 seconds during a period when no thruster activity was commanded. This produced an unplanned delta-V of 0.08 m/s and an attitude perturbation of 0.3 degrees. Telemetry analysis revealed a stuck valve in the B3 propellant control valve assembly, likely caused by particulate contamination during ground processing. The flight software detected the anomaly within 500 ms and inhibited the B3 cluster. Subsequent attitude corrections were performed using the remaining seven clusters. The propellant impact of the lost cluster reduces the available delta-V for the mission by approximately 4 m/s, leaving a remaining reserve of 218 m/s against a requirement of 150 m/s.
"#;

/// Questions designed to force deep navigation:
///
/// 1. "How much delta-V budget remains after the Day 17 thruster failure,
///     and is it enough to complete the mission?"
///     → Requires finding delta-V budget (Orbital Mechanics > Transfer > Delta-V Budget)
///     AND the anomaly impact (Mission Anomalies > Day 17 Thruster Misfire)
///     AND cross-referencing reserve vs requirement.
///
/// 2. "What is the total power generation margin at end-of-life for Satellite Alpha
///     compared to its minimum operational requirement?"
///     → Requires finding EOL power (Payload > Alpha > Solar Panel Extension)
///     and computing the difference.
///
/// 3. "If the B3 thruster cluster had failed during the Day 3 blackout instead of
///     Day 17, would the spacecraft have been able to recover attitude without
///     ground intervention?"
///     → Requires combining anomaly timelines and thruster redundancy info.
const QUERIES: &[&str] = &[
    "where can i find the backup landing zone",
];

#[tokio::main]
async fn main() -> vectorless::Result<()> {
    tracing_subscriber::fmt::init();

    println!("=== Deep Retrieval Example ===\n");

    let api_key = std::env::var("LLM_API_KEY").unwrap_or_else(|_| "sk-...".to_string());
    let model = std::env::var("LLM_MODEL").unwrap_or_else(|_| "gpt-4o".to_string());
    let endpoint = std::env::var("LLM_ENDPOINT").unwrap_or_else(|_| "https://api".to_string());

    // Build engine
    let engine = EngineBuilder::new()
        .with_key(&api_key)
        .with_model(&model)
        .with_endpoint(&endpoint)
        .build()
        .await
        .map_err(|e| vectorless::Error::Config(e.to_string()))?;

    // Index document
    let temp_dir = tempfile::tempdir()?;
    let md_path = temp_dir.path().join("mission_atlas.md");
    tokio::fs::write(&md_path, MISSION_REPORT).await?;

    let index_result = engine
        .index(IndexContext::from_path(&md_path).with_options(IndexOptions::new().with_summaries()))
        .await?;
    let doc_id = index_result.doc_id().unwrap().to_string();
    println!("Indexed document: {}\n", doc_id);

    // Query
    for query in QUERIES {
        println!("Q: \"{}\"", query);

        match engine
            .query(
                QueryContext::new(*query)
                    .with_doc_ids(vec![doc_id.clone()])
                    .with_force_analysis(true),
            )
            .await
        {
            Ok(result) => {
                if let Some(item) = result.single() {
                    if item.content.is_empty() {
                        println!("   No relevant content found");
                    } else {
                        println!("   A:");
                        for line in item.content.lines().take(10) {
                            println!("     {}", line);
                        }
                        if item.content.lines().count() > 10 {
                            println!("     ... ({} more lines)", item.content.lines().count() - 10);
                        }
                    }
                }
            }
            Err(e) => println!("   Error: {}", e),
        }
        println!();
    }

    // Cleanup
    engine.remove(&doc_id).await?;
    Ok(())
}

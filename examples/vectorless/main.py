# Copyright (c) 2026 vectorless developers
# SPDX-License-Identifier: Apache-2.0

"""Single-document reasoning challenge.

Compiles a realistic technical document and asks questions that require
the engine to navigate deep into the tree, cross-reference details
across distant sections, and extract information buried in nested
structures — not surface-level keyword matches.
"""

import asyncio
import os

from vectorless import Engine

# A research report with information scattered across sections.
# The answers to the challenge questions require connecting dots
# from different parts of the document, not simple keyword lookup.
REPORT = """
# Quantum Computing Division — Annual Research Report 2025

## Executive Summary

The Quantum Computing Division achieved several milestones in fiscal year 2025.
Total division revenue reached $47.2M, representing 23% year-over-year growth.
The division employed 312 staff across four research labs as of December 2025.
Headcount grew by 18% during the year, with the majority of new hires in the
error correction and cryogenics teams.

The board approved a $200M capital investment program spanning 2025-2028.
Phase 1 ($52M) was fully deployed in 2025, primarily in dilution refrigerator
procurement and cleanroom expansion at the Zurich facility.

## Research Labs

### Lab A — Superconducting Qubits (Zurich)

Lab A focuses on transmon qubit design and fabrication. The lab operates
two dilution refrigerators: FR-01 (purchased 2023, 20mK base temperature)
and FR-02 (commissioned Q3 2025, 15mK base temperature). FR-02 was the
single largest capital expenditure in 2025 at $8.7M.

Current qubit specifications:
- Qubit count: 127 (FR-01: 64, FR-02: 63)
- Average T1 coherence time: 142 microseconds (up from 98μs in 2024)
- Average T2 coherence time: 89 microseconds
- Single-qubit gate fidelity: 99.92%
- Two-qubit gate fidelity: 99.67%
- Readout fidelity: 99.81%

The 2025 coherence improvement was primarily driven by the transition from
aluminum to tantalum transmon junctions, which reduced two-level system (TLS)
defect density by 40%.

### Lab B — Topological Qubits (Tokyo)

Lab B pursues Majorana-based topological qubits using semiconductor-superconductor
nanowires. The team fabricated 12 nanowire devices during 2025, of which 3
demonstrated measurable topological gap. This is a significant improvement
over 2024 when only 1 device out of 8 showed the gap.

The topological gap measurement protocol requires the device temperature to
remain below 20mK throughout the 48-hour characterization cycle. Only FR-02
in Zurich meets this requirement, so Lab B ships devices to Zurich for final
characterization — creating a logistical dependency between the two labs.

Key metric: topological gap size averaged 0.35meV across successful devices,
compared to the theoretical target of 0.5meV. The gap-to-target ratio improved
from 48% in 2024 to 70% in 2025.

### Lab C — Quantum Error Correction (Cambridge)

Lab C develops surface code error correction protocols. In 2025, the team
achieved a critical milestone: below-threshold error correction on a 17-qubit
surface code patch, reducing logical error rate from 2.1×10⁻² to 3.4×10⁻³
per correction cycle.

The threshold simulations used Lab A's measured gate fidelities as input
parameters. The below-threshold result was only possible after Lab A's T1
coherence improvement from 98μs to 142μs — the simulation models showed
that the 98μs regime was above the error correction threshold for the 17-qubit
code, making the Lab A / Lab C dependency critical.

Lab C also developed a new decoder algorithm called "Cascade" that reduces
classical processing latency from 1.2μs to 0.4μs per syndrome extraction cycle.
This decoder runs on an FPGA co-processor board that was custom-designed by
Lab D.

### Lab D — Control Systems (Boston)

Lab D designs and manufactures the classical control electronics for all qubit
types. The flagship product is the QCS-4 control system, capable of driving
up to 256 qubit channels with 14-bit DAC resolution and sub-nanosecond timing
precision.

In 2025, Lab D delivered 4 QCS-4 units to Lab A and 2 units to Lab B.
Lab C received a modified QCS-4 variant with the integrated FPGA decoder
co-processor. The FPGA decoder board is a custom design: Xilinx Ultrascale+
XCU26 FPGA, 400k logic cells, running at 350MHz. Lab D is the sole source
for this board — there is no commercial equivalent.

A notable incident occurred in August 2025 when a firmware bug in the QCS-4
DAC calibration routine caused systematic phase errors in two-qubit gate
operations. The bug was traced to an integer overflow in the calibration LUT
when operating above 4.2 GHz. The issue affected Lab A's FR-01 for 11 days
before a patched firmware was deployed. During this period, Lab A's measured
two-qubit gate fidelity temporarily dropped to 97.31%.

## Financial Summary

| Category | 2024 | 2025 | Change |
|----------|------|------|--------|
| Revenue | $38.4M | $47.2M | +23% |
| R&D Expense | $31.6M | $38.9M | +23% |
| Capital Expenditure | $18.2M | $52.0M | +186% |
| Staff Count (Dec) | 264 | 312 | +18% |
| Patents Filed | 14 | 19 | +36% |

Revenue breakdown by source:
- Government contracts: $19.8M (42%)
- Enterprise partnerships: $15.3M (32%)
- IP licensing: $8.6M (18%)
- Consulting services: $3.5M (8%)

The $52M capital expenditure in 2025 included:
- FR-02 dilution refrigerator (Zurich): $8.7M
- Cleanroom expansion (Zurich): $14.2M
- Nanowire fabrication equipment (Tokyo): $6.1M
- FPGA development and QCS-4 production (Boston): $9.4M
- General infrastructure and IT: $13.6M

## Outlook for 2026

Priority goals for 2026:
1. Scale to 256 superconducting qubits by Q3 (requires a third dilution
   refrigerator, procurement estimated at $9-11M)
2. Achieve topological gap above 0.45meV (requires device process improvement)
3. Demonstrate below-threshold error correction on a 49-qubit surface code
   (requires both 256-qubit hardware AND the Cascade decoder scaling to
   larger code distances)
4. File 25+ patents
5. Grow revenue to $60M
"""

CHALLENGE_QUESTIONS = [
    # Requires: cross-reference Lab B's device characterization needs with
    # Lab A's FR-02 specs, then connect to the CapEx table for FR-02 cost
    "How much did the only refrigerator capable of characterizing Lab B's devices cost, and where is it located?",
    # Requires: trace Lab C's below-threshold result -> depends on Lab A's T1
    # improvement -> depends on tantalum junction transition
    "What specific materials change in another lab made Lab C's error correction milestone possible?",
    # Requires: find the firmware bug in Lab D section, then look at the
    # Lab A FR-01 qubit count, then compute the impact window
    "How many qubits were affected by the firmware bug, and for how many days?",
    # Requires: Lab B gap/target ratio (70%) * theoretical target (0.5meV)
    # -> actual gap = 0.35meV, compare with 2026 goal of 0.45meV
    "What is the gap between Lab B's current topological gap achievement and the 2026 target, in meV?",
    # Requires: trace the dependency chain: 256-qubit goal -> need FR-03 ->
    # cost $9-11M -> government contracts are largest revenue source at $19.8M
    "If the 2026 qubit scaling goal requires a new refrigerator, can the largest revenue source category alone cover its estimated cost?",
]


async def main() -> None:
    print("=== Single-Document Reasoning Challenge ===\n")

    api_key = os.environ.get("LLM_API_KEY", "sk-...")
    model = os.environ.get("LLM_MODEL", "gpt-4o")
    endpoint = os.environ.get("LLM_ENDPOINT", "https://api.openai.com/v1")

    engine = Engine(api_key=api_key, model=model, endpoint=endpoint)

    doc_name = "qc_report_2025"

    # Check if already compiled
    doc_id = None
    docs = await engine.list_documents()
    for doc in docs:
        if doc.name == doc_name:
            doc_id = doc.doc_id
            print(f"Document already compiled, reusing: {doc_id}\n")
            break

    if doc_id is None:
        print("Compiling research report...")
        result = await engine.compile(content=REPORT, format="markdown", name=doc_name)
        doc_id = result.doc_id
        print(f"  doc_id: {doc_id}\n")

    # Challenge queries
    for i, question in enumerate(CHALLENGE_QUESTIONS, 1):
        print(f"Q{i}: {question}")

        try:
            answer = await engine.ask(question, doc_ids=[doc_id])
            if not answer.answer:
                print("   (no answer found)\n")
            else:
                lines = answer.answer.split("\n")
                for line in lines[:3]:
                    print(f"   {line}")
                remaining = len(lines) - 3
                if remaining > 0:
                    print(f"   ... ({remaining} more lines)")
                print(f"   confidence: {answer.confidence:.2f}\n")
        except Exception as e:
            print(f"   error: {e}\n")

    # Uncomment to remove the document after testing:
    # await engine.forget(doc_id)
    # print("Cleaned up.")


if __name__ == "__main__":
    asyncio.run(main())

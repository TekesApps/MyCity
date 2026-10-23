# Dividend-to-liquidation and bounded IPC reader: finite verified stage

Issues: #10 (D) and #112 (M0 implementation prerequisite). Direct main integration; no PR.

## Actual D persistent scenario

299 and final-source304 each ran the new equity_dividend_liquidation Store test, passed.
One persistent world performs actual contributions, sales, payroll, utilities, sourced tax,
loan repayment, ordinary dividend, fiscal reuse, exit distribution and close. Two actual
10,000c contributions plus 600,100c opening paid-in make 620,100c capital. Actual company
cash before dividend is 600000+20000+1000-800-10-45=620145c (loan received and repaid).
45c dividend is paid first; treasury receives1c then spends1c. Remaining620100c is returned
10000c to treasury and610100c to the individual holder, without deducting the previous
45c dividend from capital. Company cash0, exitClosed, totalcash3000000c.

Applied-reply loss for dividend and liquidation is followed by real drop/open and exact replay.
Checkpoint/open and replay of begin/settle/close leave state hash and cash unchanged.
The full actual Store original is retained in each test-artifacts directory listed in commands.json.
checkpoint-oracle.json separately decodes retained299 checkpoint with literal account totals;
the final-source304 executable independently repeats the same full workflow.
No new production runtime rows or capacities are added by this test. New source is18346B;
full-process/Store transient memory increment is not measured here.

## IPC reader actual integration

300 ran3 focused tests (maximum64KiB frame/EOF, oversize/truncation, childkill/wait ordering);
other Cargo targets were filtered and ran zero tests, not counted as tests passed.
301 release headless build passed.302 actually ran8 sim-probe cases with fixed executable
and full retained worlds; all operation outcomesOk and persistent_reopen_verified true.
All complete-air-state hashes and business-without-revision hashes match. The report explicitly
labels this same-CPU-state comparison, not an independent numerical algorithm. Its embedded
source matches fixed-binary.json.303 workspace/all-target strict and305 fmt passed.

The two-file implementation requests512KiB reader thread stacks and on spawn failure
kills/waits owned children before supervisor/grant reconciliation. Authoritative IO remains
the bounded64KiB protocol and existing framed receipts, EOF and error paths. Trigger is
reader/process lifecycle, not a city simulation tick; no batching across frames, grant transfer
or wait boundary is permitted. Worker/Sim own readers; existing pool owns grants. The
synchronous framing and same-CPU persistent probe remain reference paths. No resource
credit is deducted for the requested stack size. The cleanup helper test does not force an
actual OS thread-builder failure.

302 time-l reported maxRSS71647232B, peakfootprint56885656B and3.57s real; report internal
wall2.958s. This is a tiny functional probe, not whole-process-group physical measurement.
Native footprint savings, original normal load, common8MiB, 8h and real16GB are not passed.
Original failure evidence and frozen load are retained; no threshold or quantity was reduced.

All commands reuse external HP P900 USB/APFS SSD target and work. Source ZIPs, actual
executables, logs, receipts and full recovery worlds are kept under construction-utilities-current-01.
The D-only299 source precedes reader integration; final304 and300-305 use the final combined
source pin, not inferred from the older isolated branch292 results.

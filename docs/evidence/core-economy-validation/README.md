# Core economy validation evidence

`final-0f2b1b.../attempt-01` is the accepted frozen source, release build, 10-test run, strict all-target Clippy, and six fixed-binary fixture pureplan runs. `source.zip` SHA-256 is `0f2b1b6228869c20417280f218e1111758ea7bd14a1b197afa9b6203d190f7e3`. The original fixed binary remains at `work/candidate-binaries/core-economy/<source>/town_economy`; its hash and hardlink identity are recorded in the evidence.

`failed-b9d9.../attempt-01` preserves the invalid-scope invocation that ascended from the evidence directory and tested the canonical checkout (0 matched tests). `attempt-02` ran the economy worktree, passed 10 tests and built the example, but strict Clippy found two diagnostics; both failures are retained as evidence. A new source freeze fixed the diagnostics and passed all gates.

All fixture runs are pure plans with test-supply inputs. Production/consumption atomic Engine settlement and shared transport supply integration remain unimplemented; this evidence does not establish a settled 28-day economy.

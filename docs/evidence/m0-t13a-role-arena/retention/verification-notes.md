# Terminal cleanup verification correction

All13 exact allowlisted files were unlinked and the3 old/input parent directories were fsynced before the strict metadata assertion. That assertion then failed because19 retained paths had only lower st_blocks, including hardlink aliases; device/inode/length/mtime/nlink unchanged. No model/builder/work/target file holder was present. The failed script source is retained as retire-stage-before-verification-fix.py.

Follow-up checked every110949 retained file identity/length/mtime/nlink, rehashed all3 complete protected layouts against29-file indexes and frozen source+AGENTS. All12 affected joint paths match their actual precleanup artifacts.zip bytes. Retained unpaced paths likewise changed allocation only; their logical metadata stayed identical, and cleanup performed no writes to any retained file. The differences are preserved in verification-metadata-differences.json; their allocation change is NOT credited to deletion.

Before/after du and df were held only in the exited controller and not durably saved before the assertion. Result intentionally sets before/df_delta null rather than reconstructing them. Deleted single-link inode allocation is known exactly from persisted plan anddeleted.json; current du/df measured afresh. Future script now persists metrics before verification. No second unlink run occurred.

Only oldv70pressure/reserve and synthetic-copy originals were removed. Oldv70two layouts and synthetic inputs no longer directly restore; newv85two and oldv50 remain complete. Logs, original index, fault/business data and fixed source/lock/executables retained.

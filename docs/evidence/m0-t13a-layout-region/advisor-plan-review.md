# Advisor finite lifecycle review

Reviewed construct/restore activation placement and single-Load process ownership. No principled blocker: activation precedes large native/table/pressure allocation; restore validates original normal, metadata/COW/external and iteration before the switch. Grant uses checked original normal minus external, no extra metadata4MiB or reserve128MiB.

Once activated, failed construct/restore cannot retry another Load or return to System in the same process; use a new child. Internal snapshot validation continues reserve borrowing, never calls full Load::restore (which activates again). Full cold load is independent. Existing index raw/Value/control prefix and prior peak remain honestly charged; startup/fragmentation OOM must be retained and cannot expand grant. Records-only must not allocate the full world. No extra unsafe/allocator mechanism requested.

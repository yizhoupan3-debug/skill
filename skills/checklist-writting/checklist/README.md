# Versioned Checklist Templates

Place generated checklist markdown files in this directory.

Naming rule:
- `cl_v1.md`
- `cl_v2.md`
- `cl_v3.md`

Compatibility rule:
- new checklist files should use the `cl_vN.md` pattern
- legacy `checklist_vN.md` files may still be read during review or migration

Core rule:
- peer points are parallel by default
- any explicit serial chain stays inside one point, no matter how long it is
- after writing the markdown, explicitly tell the user how many agents to start

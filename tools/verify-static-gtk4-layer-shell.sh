#!/usr/bin/env bash
# Verify the runtime linkage contract required by statically embedded
# gtk4-layer-shell. This works on both stripped and unstripped ELF binaries.
set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <wayscriber-binary>" >&2
    exit 2
fi

binary="$1"
[[ -f "${binary}" ]] || {
    echo "Missing Wayscriber binary: ${binary}" >&2
    exit 1
}

for tool in readelf nm awk grep; do
    command -v "${tool}" >/dev/null 2>&1 || {
        echo "Missing required command: ${tool}" >&2
        exit 1
    }
done

dynamic_section="$(readelf -d "${binary}")"
if grep -Eq 'Shared library: \[libgtk4-layer-shell\.so' <<< "${dynamic_section}"; then
    echo "${binary} still dynamically requires gtk4-layer-shell" >&2
    exit 1
fi
if ! grep -Fq 'Shared library: [libwayland-client.so.0]' <<< "${dynamic_section}"; then
    echo "${binary} does not retain libwayland-client.so.0 in DT_NEEDED" >&2
    exit 1
fi

dynamic_symbols="$(nm -D --defined-only "${binary}" | awk '{print $3}')"
required_shims=(
    wl_proxy_destroy
    wl_proxy_marshal_array_flags
    wl_proxy_marshal_flags
    wl_proxy_marshal
    wl_proxy_marshal_array
    wl_proxy_marshal_constructor
    wl_proxy_marshal_constructor_versioned
    wl_proxy_marshal_array_constructor
    wl_proxy_marshal_array_constructor_versioned
)
for symbol in "${required_shims[@]}"; do
    if ! grep -Fxq "${symbol}" <<< "${dynamic_symbols}"; then
        echo "${binary} does not export required gtk4-layer-shell shim ${symbol}" >&2
        exit 1
    fi
done

echo "Verified static gtk4-layer-shell linkage and exported Wayland shims in ${binary}"

#!/bin/sh
#
# Converts a bpt package (*.bpt) into a tarball (*.tar)
#

if [ -z "${1:-}" ]; then
	echo "Provide a \`*.bpt\` file as an argument to convert it to a \`*.tar\` file." >&2
	exit 1
fi

while [ -n "${1}" ]; do
	bpt="${1}"
	out="${bpt%.bpt}.tar"
	shift

	# Confirm expected magic number
	magic="$(head -c3 "${bpt}")"
	if [ "${magic}" != "bpt" ]; then
		echo "\`${bpt}\` does not appear to be a valid bpt file."
		exit 1
	fi
	magic_len=5

	# Get signature length
	if sig="$(tail -n128 "${bpt}" | grep -a "# sig:")"; then
		sig_len="$(( $(echo "${sig}" | wc -c) + 1 ))"
	else
		sig_len=0
	fi

	# Extract the tarball
	cat "${bpt}" |\
		tail -c+${magic_len} |\
		head -c-${sig_len} |\
		zstd --decompress |\
		cat > "${out}"

	echo "Created ${out}"
done

_bpt_abspath() {
    if [[ -z $1 ]]; then
        printf '/\n'
    elif [[ $1 == /* ]]; then
        printf '%s\n' "$1"
    else
        printf '%s/%s\n' "$PWD" "$1"
    fi
}

_bpt_root_from_words() {
    local root='/'
    local expect_root=0
    local word
    local i

    for ((i = 1; i < COMP_CWORD; i++)); do
        word=${COMP_WORDS[i]}
        if (( expect_root )); then
            root=$(_bpt_abspath "$word")
            expect_root=0
            continue
        fi
        case "$word" in
            --root-dir=*) root=$(_bpt_abspath "${word#*=}") ;;
            --root-dir|-R) expect_root=1 ;;
            -R?*) root=$(_bpt_abspath "${word#-R}") ;;
            -?*R) expect_root=1 ;;
        esac
    done

    printf '%s\n' "$root"
}

_bpt_world_entries() {
    local root=$1
    local world_file="$root/etc/bpt/world"
    [[ -r $world_file ]] || return 0
    sed -e 's/#.*$//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e '/^$/d' "$world_file" 2>/dev/null
}

_bpt_installed_pkgids() {
    local root=$1
    local dir="$root/var/lib/bpt/instpkg"
    [[ -d $dir ]] || return 0
    local path
    for path in "$dir"/*.instpkg; do
        [[ -e $path ]] || continue
        basename "$path" .instpkg
    done
}

_bpt_repo_pkgids() {
    local root=$1
    command bpt -SV -R "$root" list --repository 2>/dev/null
}

_bpt_bbuild_repo_pkgids() {
    local root=$1
    _bpt_repo_pkgids "$root" | grep ':bbuild$'
}

_bpt_subcommand_from_words() {
    local expect_value=0
    local word
    local i
    for ((i = 1; i < COMP_CWORD; i++)); do
        word=${COMP_WORDS[i]}
        if (( expect_value )); then
            expect_value=0
            continue
        fi
        case "$word" in
            --root-dir|--out-dir|--priv-key|--priv-key-passphrase-file|-R|-O|-P)
                expect_value=1
                continue
                ;;
            -R?*|-O?*|-P?*)
                continue
                ;;
            --root-dir=*|--out-dir=*|--priv-key=*|--priv-key-passphrase-file=*)
                continue
                ;;
        esac
        case "$word" in
            install|remove|upgrade|downgrade|apply|check|info|files|search|list|provides|sync|fetch|clean|build|make-repo|verify|sign)
                printf '%s\n' "$word"
                return 0
                ;;
        esac
    done
    return 1
}

_bpt_complete_from_list() {
    local cur=$1
    shift
    COMPREPLY=( $(compgen -W "$*" -- "$cur") )
}

_bpt_complete_files_ext() {
    local cur=$1
    shift
    local suffixes=("$@")
    local candidate out=()
    while IFS= read -r candidate; do
        if [[ -d $candidate ]]; then
            out+=("$candidate")
            continue
        fi
        local suffix
        for suffix in "${suffixes[@]}"; do
            if [[ $candidate == *.$suffix ]]; then
                out+=("$candidate")
                break
            fi
        done
    done < <(compgen -f -- "$cur")
    COMPREPLY=("${out[@]}")
    compopt -o filenames 2>/dev/null
}

_bpt_complete_paths() {
    local cur=$1
    COMPREPLY=( $(compgen -f -- "$cur") )
    compopt -o filenames 2>/dev/null
}

_bpt() {
    local cur prev root subcommand
    cur=${COMP_WORDS[COMP_CWORD]}
    prev=${COMP_WORDS[COMP_CWORD-1]}
    root=$(_bpt_root_from_words)
    subcommand=$(_bpt_subcommand_from_words)

    case "$prev" in
        -R|--root-dir)
            COMPREPLY=( $(compgen -d -- "$cur") )
            compopt -o dirnames 2>/dev/null
            return 0
            ;;
        -O|--out-dir)
            COMPREPLY=( $(compgen -d -- "$cur") )
            compopt -o dirnames 2>/dev/null
            return 0
            ;;
        -P|--priv-key|--priv-key-passphrase-file)
            _bpt_complete_paths "$cur"
            return 0
            ;;
    esac

    if [[ -z $subcommand ]]; then
        if [[ $cur == -* ]]; then
            _bpt_complete_from_list "$cur" \
                -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify \
                -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir \
                -R --root-dir
        else
            _bpt_complete_from_list "$cur" \
                install remove upgrade downgrade apply check info files search list provides \
                sync fetch clean build make-repo verify sign
        fi
        return 0
    fi

    if [[ $cur == -* ]]; then
        case "$subcommand" in
            install)
                _bpt_complete_from_list "$cur" -r --reinstall -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            remove)
                _bpt_complete_from_list "$cur" -p --purge -f --forget -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            check)
                _bpt_complete_from_list "$cur" -s --strict -i --ignore-backup -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            search)
                _bpt_complete_from_list "$cur" -n --name -d --description -i --installed -r --repository -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            list)
                _bpt_complete_from_list "$cur" -i --installed -r --repository -x --explicit -d --dependency -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            provides)
                _bpt_complete_from_list "$cur" -i --installed -r --repository -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            build)
                _bpt_complete_from_list "$cur" -a --arch -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            sync)
                _bpt_complete_from_list "$cur" -f --force -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            clean)
                _bpt_complete_from_list "$cur" -p --packages -s --source -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            sign)
                _bpt_complete_from_list "$cur" -n --needed -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
            *)
                _bpt_complete_from_list "$cur" -h --help -v --version -y --yes -D --dry-run -N --netutil-stderr -V --skip-verify -S --skip-sign -P --priv-key --priv-key-passphrase-file -O --out-dir -R --root-dir
                ;;
        esac
        return 0
    fi

    case "$subcommand" in
        remove)
            COMPREPLY=( $(compgen -W "$(_bpt_world_entries "$root")" -- "$cur") )
            ;;
        check)
            COMPREPLY=( $(compgen -W "$(_bpt_installed_pkgids "$root")" -- "$cur") )
            ;;
        info|files)
            COMPREPLY=( $(compgen -W "$(_bpt_installed_pkgids "$root") $(_bpt_repo_pkgids "$root")" -- "$cur") )
            if [[ ${#COMPREPLY[@]} -eq 0 || $cur == */* || $cur == ./* || $cur == ../* ]]; then
                _bpt_complete_files_ext "$cur" bpt bbuild
            fi
            ;;
        install|upgrade|downgrade)
            COMPREPLY=( $(compgen -W "$(_bpt_repo_pkgids "$root")" -- "$cur") )
            if [[ ${#COMPREPLY[@]} -eq 0 || $cur == */* || $cur == ./* || $cur == ../* ]]; then
                _bpt_complete_files_ext "$cur" bpt bbuild pkgidx fileidx
            fi
            ;;
        fetch)
            COMPREPLY=( $(compgen -W "$(_bpt_repo_pkgids "$root")" -- "$cur") )
            ;;
        build)
            COMPREPLY=( $(compgen -W "$(_bpt_bbuild_repo_pkgids "$root")" -- "$cur") )
            if [[ ${#COMPREPLY[@]} -eq 0 || $cur == */* || $cur == ./* || $cur == ../* ]]; then
                _bpt_complete_files_ext "$cur" bbuild
            fi
            ;;
        sync)
            _bpt_complete_files_ext "$cur" pkgidx fileidx
            ;;
        verify|sign)
            _bpt_complete_paths "$cur"
            ;;
        *)
            COMPREPLY=()
            ;;
    esac
}

complete -F _bpt bpt

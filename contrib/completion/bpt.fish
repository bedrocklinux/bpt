function __bpt_abspath
    set -l path $argv[1]
    if test -z "$path"
        echo /
    else if string match -q '/*' -- "$path"
        echo "$path"
    else
        echo "$PWD/$path"
    end
end

function __bpt_root
    set -l cmd (commandline -opc)
    set -l root /
    set -l expect_root 0

    for token in $cmd[2..-1]
        if test $expect_root -eq 1
            set root (__bpt_abspath "$token")
            set expect_root 0
            continue
        end
        switch $token
            case '--root-dir=*'
                set root (__bpt_abspath (string replace -- '--root-dir=' '' -- "$token"))
            case '--root-dir' '-R'
                set expect_root 1
            case '-R*'
                if test (string length -- "$token") -gt 2
                    set root (__bpt_abspath (string sub -s 3 -- "$token"))
                end
        end
        if string match -qr '^-[^-].*R$' -- "$token"
            set expect_root 1
        end
    end

    echo "$root"
end

function __bpt_world_entries
    set -l root (__bpt_root)
    set -l world_file "$root/etc/bpt/world"
    if test -r "$world_file"
        sed -e 's/#.*$//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*$//' -e '/^$/d' "$world_file" 2>/dev/null
    end
end

function __bpt_installed_pkgids
    set -l root (__bpt_root)
    set -l dir "$root/var/lib/bpt/instpkg"
    if test -d "$dir"
        for path in "$dir"/*.instpkg
            if test -e "$path"
                path basename "$path" | string replace -r '\.instpkg$' ''
            end
        end
    end
end

function __bpt_repo_pkgids
    set -l root (__bpt_root)
    command bpt -SV -R "$root" list --repository 2>/dev/null
end

function __bpt_bbuild_repo_pkgids
    __bpt_repo_pkgids | string match -r ':bbuild$'
end

function __bpt_needs_subcommand
    set -l cmd (commandline -opc)
    for token in $cmd[2..-1]
        switch $token
            case install remove upgrade downgrade apply check info files search list provides sync fetch clean build make-repo verify sign
                return 1
        end
    end
    return 0
end

complete -c bpt -n '__bpt_needs_subcommand' -f -a 'install remove upgrade downgrade apply check info files search list provides sync fetch clean build make-repo verify sign'

complete -c bpt -s h -l help -d 'Display help information'
complete -c bpt -s v -l version -d 'Display version information'
complete -c bpt -s y -l yes -d 'Assume yes as answer to all prompts'
complete -c bpt -s D -l dry-run -d 'Show steps without taking them'
complete -c bpt -s N -l netutil-stderr -d 'Print network utility stderr'
complete -c bpt -s V -l skip-verify -d 'Skip verifying signatures'
complete -c bpt -s S -l skip-sign -d 'Skip signing results'
complete -c bpt -s P -l priv-key -r -F -d 'Minisign private key'
complete -c bpt -l priv-key-passphrase-file -r -F -d 'Private key passphrase file'
complete -c bpt -s O -l out-dir -r -a '(__fish_complete_directories)' -d 'Output directory'
complete -c bpt -s R -l root-dir -r -a '(__fish_complete_directories)' -d 'Manage file system at root'

complete -c bpt -n '__fish_seen_subcommand_from install' -s r -l reinstall -d 'Reinstall already installed package(s)'
complete -c bpt -n '__fish_seen_subcommand_from remove' -s p -l purge -d 'Also remove modified configuration files'
complete -c bpt -n '__fish_seen_subcommand_from remove' -s f -l forget -d 'Forget package metadata without removing files from disk'
complete -c bpt -n '__fish_seen_subcommand_from check' -s s -l strict -d 'Treat backup differences as errors'
complete -c bpt -n '__fish_seen_subcommand_from check' -s i -l ignore-backup -d 'Ignore backup differences'
complete -c bpt -n '__fish_seen_subcommand_from info' -s i -l installed -d 'Search installed packages'
complete -c bpt -n '__fish_seen_subcommand_from info' -s r -l repository -d 'Search repository packages'
complete -c bpt -n '__fish_seen_subcommand_from files' -s i -l installed -d 'Search installed packages'
complete -c bpt -n '__fish_seen_subcommand_from files' -s r -l repository -d 'Search repository packages'
complete -c bpt -n '__fish_seen_subcommand_from search' -s n -l name -d 'Search package names'
complete -c bpt -n '__fish_seen_subcommand_from search' -s d -l description -d 'Search package descriptions'
complete -c bpt -n '__fish_seen_subcommand_from search' -s i -l installed -d 'Search installed packages'
complete -c bpt -n '__fish_seen_subcommand_from search' -s r -l repository -d 'Search repository packages'
complete -c bpt -n '__fish_seen_subcommand_from list' -s i -l installed -d 'List installed packages'
complete -c bpt -n '__fish_seen_subcommand_from list' -s r -l repository -d 'List repository packages'
complete -c bpt -n '__fish_seen_subcommand_from list' -s x -l explicit -d 'List explicit packages'
complete -c bpt -n '__fish_seen_subcommand_from list' -s d -l dependency -d 'List dependency packages'
complete -c bpt -n '__fish_seen_subcommand_from provides' -s i -l installed -d 'Search installed packages'
complete -c bpt -n '__fish_seen_subcommand_from provides' -s r -l repository -d 'Search repository packages'
complete -c bpt -n '__fish_seen_subcommand_from sync' -s f -l force -d 'Refresh indexes even if they were checked recently'
complete -c bpt -n '__fish_seen_subcommand_from clean' -s p -l packages -d 'Remove cached packages'
complete -c bpt -n '__fish_seen_subcommand_from clean' -s s -l source -d 'Remove cached source files'
complete -c bpt -n '__fish_seen_subcommand_from build' -s a -l arch -r -a 'host bbuild native noarch aarch64 armv7hl armv7l i586 i686 loongarch64 mips mips64 mips64el mipsel powerpc powerpc64 powerpc64le riscv64gc s390x x86_64' -d 'Target architecture'
complete -c bpt -n '__fish_seen_subcommand_from sign' -s n -l needed -d 'Only sign files which do not currently verify'

complete -c bpt -n '__fish_seen_subcommand_from remove' -f -a '(__bpt_world_entries)' -d 'World entry'
complete -c bpt -n '__fish_seen_subcommand_from check' -f -a '(__bpt_installed_pkgids)' -d 'Installed package'
complete -c bpt -n '__fish_seen_subcommand_from fetch' -f -a '(__bpt_repo_pkgids)' -d 'Repository package'
complete -c bpt -n '__fish_seen_subcommand_from build' -f -a '(__bpt_bbuild_repo_pkgids)' -d 'Repository bbuild'
complete -c bpt -n '__fish_seen_subcommand_from info files' -f -a '(__bpt_installed_pkgids)' -d 'Installed package'
complete -c bpt -n '__fish_seen_subcommand_from info files install upgrade downgrade fetch' -f -a '(__bpt_repo_pkgids)' -d 'Repository package'

complete -c bpt -n '__fish_seen_subcommand_from install upgrade downgrade info files' -F
complete -c bpt -n '__fish_seen_subcommand_from build' -a '(__fish_complete_suffix .bbuild)'
complete -c bpt -n '__fish_seen_subcommand_from sync' -a '(__fish_complete_suffix .pkgidx) (__fish_complete_suffix .fileidx)'
complete -c bpt -n '__fish_seen_subcommand_from verify sign' -F

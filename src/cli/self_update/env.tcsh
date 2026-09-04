# rustup environment for tcsh
if ( $?PATH ) then
    if ( "$PATH" !~ *{rustup_bin}* ) then
        setenv PATH "{rustup_bin}:$PATH"
    endif
else
    setenv PATH "{rustup_bin}"
endif

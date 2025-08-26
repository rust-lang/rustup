# rustup environment for tcsh
if ( $?PATH ) then
    if ( "$PATH" !~ *{cargo_bin}* ) then
        setenv PATH "{cargo_bin}:$PATH"
    endif
else
    setenv PATH "{cargo_bin}"
endif

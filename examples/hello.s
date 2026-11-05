; Print a string and halt.
        ENTRY
start   ADR     R0, msg         ; R0 = address of the string
        SWI     3               ; print string
        SWI     2               ; halt

msg     DEFB    "Hello, world!\n", 0
        ALIGN

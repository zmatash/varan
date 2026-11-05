; Recursive factorial using the stack and BL / LDMFD to return.
; Try "Step Over" (F10) on the BL instruction.
        ENTRY
        MOV     SP, #0x10000    ; set up the stack pointer
        MOV     R4, #1          ; n
next    MOV     R0, R4
        SWI     4               ; print n
        ADR     R0, sep
        SWI     3               ; print "! = "
        MOV     R0, R4
        BL      fact            ; R0 = n!
        SWI     4
        MOV     R0, #10
        SWI     0               ; newline
        ADD     R4, R4, #1
        CMP     R4, #10
        BLE     next
        SWI     2

; fact: R0 = R0!  (preserves all other registers)
fact    STMFD   SP!, {R4, LR}
        MOV     R4, R0
        CMP     R0, #1
        MOVLE   R0, #1
        BLE     fact_end
        SUB     R0, R0, #1
        BL      fact            ; R0 = (n-1)!
        MUL     R0, R4, R0      ; R0 = n * (n-1)!
fact_end
        LDMFD   SP!, {R4, PC}   ; restore R4 and return

sep     DEFB    "! = ", 0
        ALIGN

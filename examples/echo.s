; Read characters typed in the console and echo them back in upper case.
; Type a line in the console input box and press Enter. A '.' stops the program.
        ENTRY
loop    SWI     1               ; R0 = next character (waits for input)
        CMP     R0, #'.'
        BEQ     done
        CMP     R0, #'a'        ; convert a-z to A-Z
        BLT     print
        CMP     R0, #'z'
        SUBLE   R0, R0, #32     ; 'a' - 'A' = 32
print   SWI     0               ; print character
        B       loop

done    ADR     R0, bye
        SWI     3
        SWI     2

bye     DEFB    "\nBye!\n", 0
        ALIGN

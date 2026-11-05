; Sort an array of words in memory with bubble sort, then print it.
; Open the Memory tab and go to "array" to watch it being sorted.
count   EQU     8
        ENTRY
        ADR     R5, array
        MOV     R1, #count - 1          ; passes remaining
outer   MOV     R2, #0                  ; index
        MOV     R6, #0                  ; swapped flag
inner   ADD     R3, R5, R2, LSL #2      ; R3 = &array[i]
        LDR     R7, [R3]
        LDR     R8, [R3, #4]
        CMP     R7, R8
        STRGT   R8, [R3]                ; swap if out of order
        STRGT   R7, [R3, #4]
        MOVGT   R6, #1
        ADD     R2, R2, #1
        CMP     R2, R1
        BLT     inner
        CMP     R6, #0                  ; no swaps -> finished
        BEQ     print
        SUBS    R1, R1, #1
        BNE     outer

print   MOV     R2, #0
ploop   LDR     R0, [R5, R2, LSL #2]
        SWI     4
        MOV     R0, #' '
        SWI     0
        ADD     R2, R2, #1
        CMP     R2, #count
        BLT     ploop
        MOV     R0, #10
        SWI     0
        SWI     2

array   DEFW    42, -7, 19, 3, 100, 0, 64, 5

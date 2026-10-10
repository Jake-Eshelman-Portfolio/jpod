# Equation

Since we do not have an extra GPIO for setting the master clock (MCLK) on the DAC, we must configure it ourselves. To do that, we calculate a phase-locked loop (PLL) to set up internal DAC clocks along with the internal DAC sampling frequency. We derive the PLL's input value ($\text{PLL}_\text{CLKIN}$) from I2S's bit clock (BCLK):
$$
    \text{PLL}_\text{CLKIN} = \text{BCLK} \times \text{Channels} \times \text{Word Length}
$$
Substituting the number of channels we have (2: left and right), the word length (16), and the bit clock's frequency of 44.1 kHz, we calculate the PLL's clock input to be 1.4112 MHz for 16-bit stereo. 

To set the clocks frequency, we use the equation from the data sheet:
$$
    \text{PLL}_\text{CLK} = \frac{\text{PLL}_\text{CLKIN} \times J.D \times R}{P}.
$$
Using the data sheet's values from Table 6-28 of $P = 1$, $R = 3$, $J = 20$, and $D = 0$, we stay within the constraint
$$
    80 \text{ MHz} \le \text{PLL}_\text{CLK} \le 110 \text{ MHz}.
$$ 
*Note: we double $J$ to reach the data sheet's example row 1 since our clock input is half of their reference value.*

Now, we use this clock to set the sampling frequency inside the DAC with the equation
$$
    DAC_{f_s} = \frac{\text{PLL}_\text{CLK}}{DAC_N \times DAC_M \times DOSR},
$$
where $DAC_M = 3$, $DAC_N = 5$, and $DOSR = 128$. This leaves us with our desired frequency of 44.1 kHz.
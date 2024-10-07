# Morse Code Translator – README
Welcome to the Morse Code Translator project! This guide will help you set up, build, and run the project on your Raspberry Pi Pico.

## Step 1. Clone the Repository
First, clone the repository to your local machine:

```git clone https://github.com/GabrielaNitu/upb-fils-ma.github.io```

## Step 2. Navigate to the Project Directory
Navigate to the project directory in your terminal:

```cd path/to/project-GabrielaNitu-main/project-GabrielaNitu-main/morse_code```

## Step 3. Build the Project
Execute the following command to build the project:

```cargo build --target thumbv6m-none-eabi```

## Step 4. Connect Your Raspberry Pi Pico
Connect the Raspberry Pi Pico to your computer via USB.
Ensure the correct pins are connected. You can refer to the provided KiCad schematic or the code for the pin configuration.

## Step 5. Connect to the Server via WiFi
Ensure your Raspberry Pi Pico is connected to the same network as the server.
Once connected, your device will be assigned an IP address. 

## Step 6. Flash the Firmware
Use elf2uf2-rs to convert the ELF file to a UF2 file and flash it to your Raspberry Pi Pico:

```elf2uf2-rs -s -d .\target\thumbv6m-none-eabi\debug\morse_code```

## Step 7. Send a Message to the Server
After you have connected your Raspberry Pi Pico to the same network as the server (Step 5) and flashed the firmware (Step 6), open your command prompt and execute the following command, replacing 172.20.10.3 with the IP address of your Raspberry Pi Pico:

```echo "Any text you want" | ncat -u 172.20.10.3 1234```

Replace "Any text you want" with the message you want to send. This command sends the message to the server running on your Raspberry Pi Pico via UDP on port 1234. You should see the translation of the Morse code displayed on the LCD screen after the message is received.


Great job! You've successfully set up your Morse code translator project.



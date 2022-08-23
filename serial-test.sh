pwd
./serial  /dev/ttyS1 /dev/ttyS5  /dev/ttyS3 -r &
./serial /dev/ttyS4  /dev/ttyS0 /dev/ttyS2 --send -d "01 02 03 04" -p -i 500 -a 192.168.8.103:502

import datetime
import random
import os

def generate_txt(ticker, days=10, filename=None):
    if not filename:
        filename = f"{ticker}.txt"
        
    start_date = datetime.datetime.now() - datetime.timedelta(days=days)
    base_price = 220.0
    
    with open(filename, 'w') as f:
        # Optional header as a comment
        f.write("# ts o h l c v\n")
        
        current_time = start_date
        price = base_price
        
        while current_time < datetime.datetime.now():
            # Skip weekends
            if current_time.weekday() >= 5:
                current_time += datetime.timedelta(days=1)
                current_time = current_time.replace(hour=9, minute=30)
                continue
                
            # Market hours (approximate for generation)
            # UTC generation: 14:30 (09:30 ET) to 21:00 (16:00 ET)
            day_start = current_time.replace(hour=14, minute=30, second=0, microsecond=0)
            day_end = current_time.replace(hour=21, minute=0, second=0, microsecond=0)
            
            if current_time < day_start:
                current_time = day_start
                
            if current_time >= day_end:
                 current_time += datetime.timedelta(days=1)
                 current_time = current_time.replace(hour=14, minute=30)
                 continue
                 
            # Generate minute bar
            o = price
            change = random.uniform(-0.5, 0.5)
            c = o + change
            h = max(o, c) + random.uniform(0, 0.2)
            l = min(o, c) - random.uniform(0, 0.2)
            v = random.randint(1000, 50000)
            
            # Format: space separated
            ts_str = current_time.strftime('%Y-%m-%dT%H:%M:%SZ')
            # line: TS O H L C V
            f.write(f"{ts_str} {o:.2f} {h:.2f} {l:.2f} {c:.2f} {v}\n")
            
            price = c
            current_time += datetime.timedelta(minutes=1)

    print(f"Generated {filename}")

if __name__ == "__main__":
    generate_txt("AMZN")

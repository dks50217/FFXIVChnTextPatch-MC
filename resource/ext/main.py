import os
import pandas as pd

def clear_character_in_csv(directory, char_to_clear):
    for root, _, files in os.walk(directory):
        for file in files:
            if file.endswith('.csv'):
                file_path = os.path.join(root, file)
                try:
                    df = pd.read_csv(file_path)
                except pd.errors.ParserError:
                    print(f"Error parsing file: {file_path}")
                    continue

                # Replace the specific character with an empty string
                df = df[~df.applymap(lambda x: char_to_clear in str(x)).any(axis=1)]

                # Save the modified DataFrame back to the CSV file
                df.to_csv(file_path, index=False)

directory_path = r'C:\Users\User\Desktop\Mic\FFXIVChnTextPatch-GP-MC\resource\rawexd\quest'
clear_character_in_csv(directory_path, '★')
import json

def convert_json_to_jsonl(input_file, output_file):
    # Read the JSON array
    with open(input_file, 'r') as f:
        data = json.load(f)

    # Write each object on a separate line
    with open(output_file, 'w') as f:
        for item in data:
            json.dump(item, f)
            f.write('\n')

if __name__ == '__main__':
    convert_json_to_jsonl('user_table_pretty.json', 'user_table_output.json')

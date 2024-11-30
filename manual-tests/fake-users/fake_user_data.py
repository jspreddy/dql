import faker
from dynamo3 import DynamoDBConnection
from rich.console import Console
console = Console()

ddb = DynamoDBConnection.connect(
    region="us-west-1",
    # host="localhost",
    # port=8000,
    # is_secure=False,
)

def generate_fake_user_data(num_users):
    for i in range(num_users):
        fake = faker.Faker()
        age = fake.random_int(min=1, max=100)
        dob = fake.date_of_birth(minimum_age=age, maximum_age=age).strftime("%Y-%m-%d")
        user = {
            "id": fake.user_name() + str(fake.random_int(10, 99)),
            "firstname": fake.first_name(),
            "lastname": fake.last_name(),
            "age": age,
            "dob": dob,
            "address": fake.street_address() + ", " + fake.city() + ", " + fake.state_abbr() + " " + fake.zipcode(),
            "organ_donor_status": fake.boolean(),
            "profession": fake.job(),
            "work_orgs": [fake.company() for _ in range(fake.random_int(1,2))],
            "businesses": [{"name": fake.company(), "type": fake.bs()} for _ in range(fake.random_int(0,2))],
            "banks": [fake.company() for _ in range(fake.random_int(1,3))],
            "email": fake.email(),
            "phone": fake.phone_number(),
            "education": {
                "degree": fake.random_element(elements=("Bachelors", "Masters", "PhD")) + " in " + fake.bs(),
                # "university": fake.university(),
                "graduation_year": fake.random_int(min=1900, max=2023)
            },
            "skills": [fake.job() for _ in range(fake.random_int(3,6))],
            "languages": fake.random_elements(elements=("English", "Spanish", "French", "German", "Mandarin", "Hindi"), length=fake.random_int(1,3)),
            "certifications": [fake.bs() + " Certificate" for _ in range(fake.random_int(1,3))],
            "social_media": {
                "linkedin": fake.user_name(),
                "github": fake.user_name(),
                "twitter": "@" + fake.user_name()
            },
            "hobbies": [fake.bs() for _ in range(fake.random_int(2,4))],
            "marital_status": fake.random_element(elements=("single", "married", "divorced")),
            "emergency_contact": {
                "name": fake.name(),
                "relationship": fake.random_element(elements=("spouse", "parent", "sibling")),
                "phone": fake.phone_number()
            }
        }
        yield user, i


if __name__ == '__main__':
    num_users = 10000
    batch = []

    with ddb.batch_write('users-fake-data') as batch_writer:
        for u, i in generate_fake_user_data(num_users):
            console.log(f'{i} of {num_users}: {u["id"]}')
            batch_writer.put(u)

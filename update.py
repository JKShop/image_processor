import base64
import json

import requests

URL = "https://portainer.soontm.net/api"
JWT_TOKEN = ""


def login():
    global JWT_TOKEN
    login_payload = {'username': 'admin',
                     'password': 'UzZTb2lTVXhFeTRXZVhjSl5wWFFMTm9wMmZvKnQjenZlVipBdnFrdiRUTXZoRldCSEBvUXNaMkNW'}
    response = requests.post(URL + "/auth", json=login_payload)
    if response.status_code != 200:
        print("Login request failed !")
        print(response.json())
        exit(1)
    JWT_TOKEN = response.json()["jwt"]


def get_endpoints(name):
    headers = {"Authorization": "Bearer " + JWT_TOKEN}
    response = requests.get(URL + "/stacks?filters={\"EndpointID\":1}", headers=headers)
    if response.status_code != 200:
        print("get_endpoints request failed !")
        print(response.json())
        exit(1)
    for endpoint in response.json():
        if endpoint["Name"] == name:
            return endpoint


def get_compose_file(ep_id):
    headers = {"Authorization": "Bearer " + JWT_TOKEN}
    url = URL + "/stacks/" + str(ep_id) + "/file"
    response = requests.get(url, headers=headers)
    if response.status_code != 200:
        print("get_compose_file request failed !")
        print(response.json())
        exit(1)
    return response.json()["StackFileContent"]


def update_endpoint(ep_id, compose_file):
    headers = {"Authorization": "Bearer " + JWT_TOKEN}
    url = URL + "/stacks/" + str(ep_id) + "?endpointId=1"
    payload = {
        'Env': [],
        'Prune': False,
        'StackFileContent': compose_file,
        'id': ep_id
    }
    response = requests.put(url, headers=headers, json=payload)
    if response.status_code != 200:
        print("update_endpoint request failed !")
        print(response.json())
        exit(1)


def get_image(img_name):
    headers = {"Authorization": "Bearer " + JWT_TOKEN}
    response = requests.get(URL + "/endpoints/1/docker/images/json?all=0", headers=headers)
    if response.status_code != 200:
        print("get_images request failed !")
        print(response.json())
        exit(1)
    for docker_image in response.json():
        if docker_image["RepoTags"] is not None and docker_image["RepoTags"][0] == img_name:
            return docker_image


def update_image(image):
    repo_tag = str(image["RepoTags"][0]).strip().replace("\n", "").replace("\r", "")
    registry_url = repo_tag.split("/")[0]
    url = URL + "/endpoints/1/docker/images/create?fromImage=" + repo_tag
    headers = {
        "Authorization": "Bearer " + JWT_TOKEN,
        "X-Registry-Auth": base64.b64encode(
            json.dumps(
                {
                    "serveraddress": registry_url
                }
            ).replace(" ", "").encode('utf-8')
        ).decode('utf-8')
    }
    payload = {
        "fromImage": repo_tag
    }

    response = requests.post(url, headers=headers, json=payload)
    if response.status_code != 200:
        print(response.request.body)
        print(response.request.url)
        print(response.request.headers)
        print("get_images request failed !")
        print(response.json())
        exit(1)

    for line in response.text.splitlines():
        line = json.loads(line)
        print(line["status"])


if __name__ == "__main__":
    login()
    endpoint = get_endpoints("jknewshop")
    image = get_image("registry.soontm.net/jkshop/image_processor:latest")
    update_image(image)
    compose_file = get_compose_file(endpoint["Id"])
    update_endpoint(endpoint["Id"], compose_file)


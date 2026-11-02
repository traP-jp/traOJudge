"""CloudFormation owns every MicroVM, including termination on resource deletion."""
import hashlib
import json
import time
import urllib.request
import urllib.error

import boto3


def token(client, identifier):
    result = client.create_microvm_auth_token(microvmIdentifier=identifier,
        expirationInMinutes=60, allowedPorts=[{'port': 8080}])['authToken']
    return result if isinstance(result, str) else max(result.values(), key=len)


def request(endpoint, auth, body=None):
    url = endpoint if endpoint.startswith('https://') else 'https://' + endpoint
    req = urllib.request.Request(url + ('/run' if body is not None else '/info'),
        data=json.dumps(body).encode() if body is not None else None,
        headers={'X-aws-proxy-auth': auth, 'Content-Type': 'application/json'})
    try:
        return json.loads(urllib.request.urlopen(req, timeout=120).read())
    except urllib.error.HTTPError as exc:
        raise RuntimeError('Guest HTTP failure: ' + exc.read().decode()[:1000]) from None


def terminate(client, identifier):
    try:
        client.terminate_microvm(microvmIdentifier=identifier)
        for _ in range(90):
            state = client.get_microvm(microvmIdentifier=identifier)['state']
            if state in ['TERMINATED', 'DELETED']:
                return
            time.sleep(.3)
        raise TimeoutError('MicroVM did not terminate')
    except client.exceptions.ResourceNotFoundException:
        pass


def handler(event, context):
    client = boto3.client('lambda-microvms')
    props = event['ResourceProperties']
    identifier = event.get('PhysicalResourceId')
    if event['RequestType'] == 'Delete':
        if identifier and identifier.startswith(('microvm-', 'mvm-')):
            terminate(client, identifier)
        return {'PhysicalResourceId': identifier or 'deleted'}
    if event['RequestType'] == 'Update':
        raise ValueError('Recreate this experiment stack instead of updating live VMs')
    started = time.monotonic()
    try:
        region = boto3.Session().region_name
        result = client.run_microvm(imageIdentifier=props['Image'], imageVersion=props['Version'],
            ingressNetworkConnectors=[f'arn:aws:lambda:{region}:aws:network-connector:aws-network-connector:ALL_INGRESS'],
            egressNetworkConnectors=[props['Egress']],
            maximumDurationInSeconds=3300, logging={'disabled': {}},
            clientToken=hashlib.sha256(event['RequestId'].encode()).hexdigest()[:32])
        identifier = result['microvmId']
        while result['state'] not in ['RUNNING', 'FAILED', 'TERMINATED']:
            if time.monotonic() - started > 100:
                raise TimeoutError('MicroVM did not become running')
            time.sleep(0.2)
            result = client.get_microvm(microvmIdentifier=identifier)
        if result['state'] != 'RUNNING':
            raise RuntimeError('MicroVM failed to start')
        endpoint = result['endpoint']
        auth = token(client, identifier)
        cold = request(endpoint, auth, {'compile': True, 'iterations': 100_000_000})
        evidence = {'label': props['Label'], 'run_to_first_result_seconds': time.monotonic() - started,
                    'first_submission': cold}
        boto3.client('s3').put_object(Bucket=props['Bucket'], Key=f"launch/{props['Label']}.json",
            Body=json.dumps(evidence).encode())
        return {'PhysicalResourceId': identifier, 'Data': {'Endpoint': endpoint}}
    except Exception:
        if identifier:
            terminate(client, identifier)
        raise

"""Run after CDK synth; checks isolation, lifecycle and least-privilege boundaries."""
import json
from pathlib import Path
import unittest

class StackTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        templates=list((Path(__file__).parents[1]/"cdk.out").glob("*-Judge.template.json"))
        if len(templates)!=1:raise RuntimeError("synth this branch's Judge stack first")
        cls.resources=json.loads(templates[0].read_text())["Resources"]
    def test_every_resource_is_deleted(self):
        for name,r in self.resources.items():
            self.assertEqual(r.get("DeletionPolicy"),"Delete",name)
    def test_custom_resource_logs_are_also_stack_owned(self):
        for resource in self.resources.values():
            if resource["Type"]=="AWS::Lambda::Function":
                self.assertIn("LogGroup", resource["Properties"]["LoggingConfig"])
    def test_versioned_private_bucket_and_private_database(self):
        buckets=[r["Properties"] for r in self.resources.values() if r["Type"]=="AWS::S3::Bucket"]
        self.assertEqual(len(buckets),1)
        self.assertEqual(buckets[0]["VersioningConfiguration"]["Status"],"Enabled")
        self.assertTrue(all(buckets[0]["PublicAccessBlockConfiguration"].values()))
        database=next(r["Properties"] for r in self.resources.values() if r["Type"]=="AWS::RDS::DBInstance")
        self.assertFalse(database["PubliclyAccessible"])
        self.assertTrue(database["StorageEncrypted"])
    def test_workers_have_no_ingress_and_cannot_read_database_credentials(self):
        worker=next(r["Properties"] for name,r in self.resources.items() if name.startswith("WorkerSecurity") and r["Type"]=="AWS::EC2::SecurityGroup")
        self.assertFalse(worker.get("SecurityGroupIngress"))
        policy=next(r["Properties"]["PolicyDocument"] for name,r in self.resources.items() if name.startswith("WorkerRoleDefaultPolicy"))
        self.assertNotIn("secretsmanager",json.dumps(policy))
        self.assertNotIn("rds-db",json.dumps(policy))
        for r in self.resources.values():
            if r["Type"]=="AWS::EC2::LaunchTemplate":
                self.assertEqual(r["Properties"]["LaunchTemplateData"]["MetadataOptions"]["HttpTokens"],"required")
    def test_alb_can_reach_controller_on_target_port(self):
        group=next(r["Properties"] for name,r in self.resources.items() if name.startswith("InternalApiSecurityGroup") and r["Type"]=="AWS::EC2::SecurityGroup")
        inline=group.get("SecurityGroupEgress",[])
        standalone=[r["Properties"] for r in self.resources.values() if r["Type"]=="AWS::EC2::SecurityGroupEgress"]
        rules=inline+standalone
        self.assertTrue(any(r.get("IpProtocol")=="tcp" and r.get("FromPort")==8080 and r.get("ToPort")==8080 for r in rules))
    def test_api_is_internal_and_queues_have_dead_letters(self):
        lb=next(r["Properties"] for r in self.resources.values() if r["Type"]=="AWS::ElasticLoadBalancingV2::LoadBalancer")
        self.assertEqual(lb["Scheme"],"internal")
        queues=[r["Properties"] for r in self.resources.values() if r["Type"]=="AWS::SQS::Queue"]
        self.assertEqual(len(queues),4)
        self.assertEqual(sum("RedrivePolicy" in q for q in queues),2)

if __name__=="__main__":unittest.main()

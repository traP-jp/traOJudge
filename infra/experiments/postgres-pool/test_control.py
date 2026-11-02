"""Regression for the reported stale scale-down race, with real PG locking.

Run with DATABASE_URL pointing at the disposable local PostgreSQL and pg8000 /
 boto3 installed. AWS calls and business commands are deliberately faked.
"""
import contextlib
import importlib.util
import os
from pathlib import Path
import threading
import time
import unittest
import urllib.parse
import uuid
from unittest.mock import patch

HERE=Path(__file__).resolve().parent


def load():
    spec=importlib.util.spec_from_file_location('control_under_test',HERE/'control.py')
    module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
    module.initialized=True
    return module


class ControlTest(unittest.TestCase):
    def test_event_request_reaches_cursor_reader(self):
        module=load();calls=[]
        def run(*args):calls.append(args);return {'cursor':8,'events':[{'kind':'result.published','payload':{'verdict':'AC'}}]}
        with patch.object(module,'run',run):
            result=module.handler({'action':'events','after':7},None)
        self.assertEqual(calls,[('events','global','--after','7')])
        self.assertEqual(result['events'][0]['payload']['verdict'],'AC')

    def test_real_pg_lock_serializes_acceptance_with_scale_to_zero(self):
        module=load();url=urllib.parse.urlsplit(os.environ['DATABASE_URL'])
        original=module.pg8000.native.Connection
        reached=threading.Event();release=threading.Event();accepted=threading.Event()
        state={'active':0,'desired':1};errors=[]
        def connection(**ignored):
            return original(user=urllib.parse.unquote(url.username),password=urllib.parse.unquote(url.password),
                host=url.hostname,port=url.port,database=url.path.strip('/'),ssl_context=False,timeout=10)
        def run(action,*args):
            if action=='submit':state['active']=1;accepted.set();return {'id':args[0],'ordinal':1}
            if action=='status':return {'submissions':[{'terminal':False}]*state['active']}
        class ASG:
            def describe_auto_scaling_groups(self,**args):return {'AutoScalingGroups':[{'DesiredCapacity':state['desired']}]}
            def set_desired_capacity(self,**args):
                if args['DesiredCapacity']==0:
                    reached.set()
                    if not release.wait(3):raise TimeoutError('test barrier')
                state['desired']=args['DesiredCapacity']
        def invoke(event):
            try:module.handler(event,None)
            except Exception as error:errors.append(error)
        with patch.dict(os.environ,{'DB_HOST':'test','DB_USER':'test','DB_PASSWORD':'test','ASG':'owned-test'}), \
             patch.object(module.pg8000.native,'Connection',connection), \
             patch.object(module.ssl,'create_default_context',lambda **kw:False), \
             patch.object(module,'run',run),patch.object(module.boto3,'client',lambda *a,**kw:ASG()):
            idle=threading.Thread(target=invoke,args=({'action':'status'},));idle.start()
            self.assertTrue(reached.wait(3))
            submit=threading.Thread(target=invoke,args=({'action':'submit','id':str(uuid.uuid4()),'scope':'global','fixture':{}},));submit.start()
            try:
                time.sleep(.1)
                self.assertFalse(accepted.is_set(),'admission overtook the idle scaling transaction')
            finally:release.set()
            idle.join(5);submit.join(5)
            self.assertFalse(idle.is_alive() or submit.is_alive())
        if errors:raise errors[0]
        self.assertEqual(state,{'active':1,'desired':1})


if __name__=='__main__':unittest.main()

import unittest
from quiescent_audit import inside,overlaps,layout_exclusions,idle_boundaries
class Boundaries(unittest.TestCase):
 def test_mapping_uncertainty_excludes_crossing(self):
  self.assertTrue(inside({'clock_begin_ns':150,'clock_end_ns':180},(10,20),(130,170)))
  self.assertFalse(inside({'clock_begin_ns':149,'clock_end_ns':180},(10,20),(130,170)))
 def test_snapshot_overlap_includes_edges(self):
  self.assertTrue(overlaps({'clock_begin_ns':15,'clock_end_ns':20},(20,30)))
 def test_layout_clock_is_not_elapsed_epoch(self):
  rows=[{'scan':{},'clock_begin':{'elapsed_ns':0},'clock_end':{'elapsed_ns':10},'elapsed_seconds':0.000000015},{'snapshot_admitted':{'generation':1},'elapsed_seconds':0.000000020},{'snapshot_durable':{'generation':1},'elapsed_seconds':0.000000030}]
  spans,delta=layout_exclusions(rows,(100,110))
  self.assertEqual(delta,(-6,0));self.assertEqual(spans,[(113,141)])
 def test_unfinished_snapshot_excludes_future(self):
  rows=[{'scan':{},'clock_begin':{'elapsed_ns':0},'clock_end':{'elapsed_ns':10},'elapsed_seconds':0.000000015},{'snapshot_admitted':{'generation':1},'elapsed_seconds':0.000000020}]
  spans,_=layout_exclusions(rows,(100,110));self.assertEqual(spans[0][1],10**40)
 def test_only_released_report_opens_gap(self):
  transcript=[{'elapsed_seconds':0,'data':{'experiment_origin_measurement_ns':10}},{'elapsed_seconds':1,'data':{'steady_quiescent_day_boundary':1,'status':{}}},{'elapsed_seconds':2,'data':{'request':1,'command':{}}}]
  days,gaps=idle_boundaries(transcript,300);self.assertEqual(len(days),1);self.assertEqual(gaps,[])
 def test_actual_next_request_closes_gap(self):
  transcript=[{'elapsed_seconds':0,'data':{'experiment_origin_measurement_ns':10}},{'elapsed_seconds':1,'data':{'steady_quiescent_day_boundary':1,'status':{}}},{'elapsed_seconds':1.1,'data':{'steady_quiescent_day_released':1,'phase':'released','game_target':86400}},{'elapsed_seconds':2,'data':{'request':1,'command':{}}}]
  _,gaps=idle_boundaries(transcript,300);self.assertEqual(gaps[0]['gap_driver_ns'],[1100000311,2000000309])
if __name__=='__main__':unittest.main()

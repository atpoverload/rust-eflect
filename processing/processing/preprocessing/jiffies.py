""" Processing for /proc/stat data using pandas. """

import pandas as pd

from processing.preprocessing.util import bucket_timestamps
from processing.preprocessing.util import max_rolling_difference

def parse_cpu_samples(samples):
    """ Converts a collection of CpuSample to a DataFrame. """
    records = []
    for sample in samples:
        for stat in sample.stats:
            records.append([
                sample.timestamp,
                stat.cpu,
                stat.user,
                stat.nice,
                stat.system,
                stat.idle,
                stat.iowait,
                stat.irq,
                stat.softirq,
                stat.steal,
                stat.guest,
                stat.guest_nice
            ])
    df = pd.DataFrame(records)
    df.columns = [
        'timestamp',
        'cpu',
        'user',
        'nice',
        'system',
        'idle',
        'iowait',
        'irq',
        'softirq',
        'steal',
        'guest',
        'guest_nice'
    ]
    df.timestamp = pd.to_datetime(df.timestamp, unit='ms')
    return df

def process_cpu_data(df):
    """ Computes the cpu jiffy rate of each 50ms bucket """
    df['jiffies'] = df.drop(columns = ['timestamp', 'cpu', 'idle', 'iowait']).sum(axis = 1)
    df.timestamp = bucket_timestamps(df.timestamp)

    jiffies, ts = max_rolling_difference(df.groupby(['timestamp', 'cpu']).jiffies.min().unstack())
    jiffies = jiffies.stack().reset_index()
    jiffies = jiffies.groupby(['timestamp', 'cpu']).sum().unstack()
    jiffies = jiffies.div(ts, axis = 0).stack()

    return jiffies[0]

def cpu_samples_to_df(samples):
    """ Converts a collection of CpuSamples to a processed DataFrame. """
    return process_cpu_data(parse_cpu_samples(samples))

def parse_task_samples(samples):
    """ Converts a collection of TaskSamples to a DataFrame. """
    records = []
    for sample in samples:
        for stat in sample.stats:
            records.append([
                sample.timestamp,
                stat.task_id,
                # stat.thread_name,
                stat.cpu,
                stat.user,
                stat.system
            ])
    df = pd.DataFrame(records)
    df.columns = [
        'timestamp',
        'id',
        # 'name',
        'cpu',
        'user',
        'system',
    ]
    df.timestamp = pd.to_datetime(df.timestamp, unit='ms')
    return df

def process_task_data(df):
    """ Computes the app jiffy rate of each 50ms bucket """
    df['jiffies'] = df.user + df.system
    # the thread name is currently unused because it typically isn't useful
    # df = df[~df.name.str.contains('eflect-')]
    # df['id'] = df.id.astype(str) + '-' + df.name

    df.timestamp = bucket_timestamps(df.timestamp)
    cpu = df.groupby(['timestamp', 'id']).cpu.max()

    jiffies, ts = max_rolling_difference(df.groupby(['timestamp', 'id']).jiffies.min().unstack())
    jiffies = jiffies.stack().to_frame()
    jiffies['cpu'] = cpu
    jiffies = jiffies.groupby(['timestamp', 'id', 'cpu'])[0].sum().unstack().unstack().div(ts, axis = 0).stack().stack(0)

    return jiffies

def task_samples_to_df(samples):
    """ Converts a collection of TaskSamples to a processed DataFrame. """
    return process_task_data(parse_task_samples(samples))

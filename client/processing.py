""" Code used to process data collected by eflect. """
import os

from argparse import ArgumentParser

import numpy as np
import pandas as pd

from pandas import to_datetime

from protos.sample.sample_pb2 import DataSet

# processing helpers
SAMPLE_INTERVAL = '50ms'
WINDOW_SIZE = '501ms'

def bucket_timestamps(timestamps, sample_interval=SAMPLE_INTERVAL):
    """ Floors a series of timestamps to some interval for easy aggregates. """
    return to_datetime(timestamps).dt.floor(sample_interval)

def max_rolling_difference(df, window_size = WINDOW_SIZE):
    """ Computes a rolling difference of points up to the window size. """
    values = df - df.rolling(window_size).min()

    timestamps = df.reset_index().timestamp.astype(int) / 10**9
    timestamps.index = df.index
    timestamps = timestamps - timestamps.rolling(window_size).min()

    return values, timestamps

# jiffies processing
def parse_cpu_samples(samples):
    """ Converts a collection of CpuSample to a DataFrame. """
    records = []
    for sample in samples:
        for stat in sample.stat:
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
        for stat in sample.stat:
            records.append([
                sample.timestamp,
                stat.task_id,
                # stat.task_name,
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

# rapl processing
WRAP_AROUND_VALUE = 16384

def parse_rapl_samples(samples):
    """ Converts a collection of RaplSamples to a DataFrame. """
    records = []
    for sample in samples:
        for reading in sample.reading:
            records.append([
                sample.timestamp,
                reading.socket,
                reading.cpu,
                reading.package,
                reading.dram,
                reading.gpu
            ])
    df = pd.DataFrame(records)
    df.columns = [
        'timestamp',
        'socket',
        'cpu',
        'package',
        'dram',
        'gpu'
    ]
    df.timestamp = pd.to_datetime(df.timestamp, unit='ms')
    return df

# TODO(timur): i've been told by alejandro that the value i'm using isn't
#   actually the wrap around. i'm not sure how to look it up properly however.
def maybe_apply_wrap_around(value):
    """ Checks if the value needs to be adjusted by the wrap around. """
    if value < 0:
        return value + WRAP_AROUND_VALUE
    else:
        return value

def process_rapl_data(df):
    """ Computes the power of each 50ms bucket """
    df.timestamp = bucket_timestamps(df.timestamp)
    df = df.groupby(['timestamp', 'socket']).min()
    df.columns.name = 'component'

    energy, ts = max_rolling_difference(df.unstack())
    energy = energy.stack().stack().apply(maybe_apply_wrap_around)
    energy = energy.groupby(['timestamp', 'socket', 'component']).sum().div(ts, axis = 0)

    return energy

def rapl_samples_to_df(samples):
    """ Converts a collection of RaplSamples to a processed DataFrame. """
    return process_rapl_data(parse_rapl_samples(samples))

# accounting
# TODO(timur): find out if there's a general conversion formula
DOMAIN_CONVERSION = lambda x: 0 if int(x) < 20 else 1

def account_jiffies(task, cpu):
    """ Returns the ratio of the jiffies with a correction for overaccounting. """
    task = task_samples_to_df(task)
    cpu = cpu_samples_to_df(cpu)
    # TODO(timur): let's clean this; i think it's outputting some garbage data
    return (task / cpu.replace(0, 1)).replace(np.inf, 1).clip(0, 1)

def account_rapl_energy(activity, rapl):
    """ Returns the product of the energy and the cpu-aligned activity data. """
    activity = activity.reset_index()
    activity['socket'] = activity.cpu.apply(DOMAIN_CONVERSION)
    activity = activity.set_index(['timestamp', 'id', 'socket'])[0]

    rapl = rapl_samples_to_df(rapl)

    # TODO(timur): we should just be able to take the product but the axis
    #   misalignment causes it to fail sometimes
    try:
        df = rapl * activity
    except:
        activity = activity.reset_index()
        rapl_energy = rapl_energy.reset_index()
        df = pd.merge(activity, rapl_energy, on=['timestamp', 'socket'])
        df[0] = df['0_x'] * df['0_y']
        df = df.set_index(['timestamp', 'id', 'component', 'socket'])[0]

    return df.reset_index().set_index(['timestamp', 'id', 'component', 'socket'])

def compute_footprint(data):
    """ Produces an energy footprint from the data set. """
    df = account_jiffies(data.task, data.cpu)
    df.name = 'activity'
    if len(data.rapl) > 0:
        df = account_rapl_energy(activity, data.rapl)
    return df

# cli to process globs of files
def parse_args():
    """ Parses client-side arguments. """
    parser = ArgumentParser()
    parser.add_argument(
        dest='files',
        nargs='*',
        default=None,
        help='files to process'
    )
    parser.add_argument(
        '-o',
        '--output_dir',
        dest='output',
        default=None,
        help='path to write the data to'
    )
    return parser.parse_args()

def main():
    args = parse_args()
    for file in args.files:
        with open(file, 'rb') as f:
            data = DataSet()
            data.ParseFromString(f.read())
        # TODO(timur): i hate that i did this. we need to get the footprint in the proto
        if args.output:
            if os.path.exists(args.output) and not os.path.isdir(args.output):
                raise RuntimeError('output target {} already exists and is not a directory; aborting'.format(args.output))
            elif not os.path.exists(args.output):
                os.makedirs(args.output)

            path = os.path.join(args.output, os.path.splitext(os.path.basename(file))[0] + '-footprint.csv')
        else:
            path = os.path.splitext(file)[0] + '-footprint.csv'
        compute_footprint(data).to_csv(path)
        print('wrote footprint for data set {} at {}'.format(file, path))

if __name__ == '__main__':
    main()
